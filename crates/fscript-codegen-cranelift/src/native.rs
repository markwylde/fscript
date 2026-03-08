use std::{collections::BTreeMap, fs, process::Command};

use camino::Utf8Path;
use cranelift_codegen::{
    ir,
    ir::{AbiParam, InstBuilder, UserFuncName, types},
    settings::{self, Configurable},
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};
use fscript_ir::{BinaryOperator, BlockItem, Expr, ModuleItem, Pattern, UnaryOperator};

use crate::CompileError;

pub(crate) fn supports_program(
    modules: &BTreeMap<String, fscript_ir::Module>,
    entry: &str,
) -> bool {
    if modules.len() != 1 {
        return false;
    }

    let Some(module) = modules.get(entry) else {
        return false;
    };

    if !module.exports.is_empty() {
        return false;
    }

    module.items.iter().all(item_is_supported)
}

pub(crate) fn compile_program(
    modules: &BTreeMap<String, fscript_ir::Module>,
    entry: &str,
    output: &Utf8Path,
) -> Result<(), CompileError> {
    if let Some(parent) = output.parent().filter(|parent| !parent.as_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|source| CompileError::CreateOutputDirectory {
            path: parent.to_owned(),
            source,
        })?;
    }

    let module = modules
        .get(entry)
        .ok_or_else(|| CompileError::NativeModule {
            details: format!("entry module `{entry}` is not available"),
        })?;

    let temp_dir = super::create_temp_directory()?;
    let object_path = temp_dir.join("program.o");
    let runtime_library_path = build_runtime_abi_library(&temp_dir)?;

    let emitted = emit_object(module, &object_path)?;
    fs::write(&object_path, emitted).map_err(|source| CompileError::ObjectEmission {
        path: object_path.clone(),
        details: source.to_string(),
    })?;

    let linker = Command::new("cc")
        .arg(&object_path)
        .arg(&runtime_library_path)
        .arg("-o")
        .arg(output)
        .output()
        .map_err(|source| CompileError::LinkInvocation {
            output: output.to_owned(),
            source,
        })?;

    if !linker.status.success() {
        return Err(CompileError::LinkFailed {
            output: output.to_owned(),
            stderr: String::from_utf8_lossy(&linker.stderr).trim().to_owned(),
        });
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

fn emit_object(
    module: &fscript_ir::Module,
    object_path: &Utf8Path,
) -> Result<Vec<u8>, CompileError> {
    let mut flag_builder = settings::builder();
    flag_builder.set("is_pic", "true").map_err(|error| {
        CompileError::NativeTargetConfiguration {
            details: error.to_string(),
        }
    })?;
    let flags = settings::Flags::new(flag_builder);
    let isa_builder =
        cranelift_native::builder().map_err(|error| CompileError::NativeTargetConfiguration {
            details: error.to_string(),
        })?;
    let isa =
        isa_builder
            .finish(flags)
            .map_err(|error| CompileError::NativeTargetConfiguration {
                details: error.to_string(),
            })?;
    let object_builder =
        ObjectBuilder::new(isa, "fscript", default_libcall_names()).map_err(|error| {
            CompileError::NativeModule {
                details: error.to_string(),
            }
        })?;
    let mut module_ctx = ObjectModule::new(object_builder);

    let pointer_type = module_ctx.target_config().pointer_type();
    let box_number = declare_value_from_number(&mut module_ctx)?;
    let print_value = declare_value_print(&mut module_ctx, pointer_type)?;
    let drop_value = declare_value_drop(&mut module_ctx, pointer_type)?;

    let main_id = declare_main(&mut module_ctx)?;
    let mut context = module_ctx.make_context();
    context.func.name = UserFuncName::user(0, main_id.as_u32());
    context
        .func
        .signature
        .returns
        .push(AbiParam::new(types::I32));

    let mut builder_context = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
        let entry_block = builder.create_block();
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let mut bindings = BTreeMap::new();
        let mut last_value = None;

        for item in &module.items {
            let ModuleItem::Binding(binding) = item else {
                return Err(CompileError::NativeModule {
                    details: "native Cranelift subset does not support imports".to_owned(),
                });
            };

            let Pattern::Identifier { name, .. } = &binding.pattern else {
                return Err(CompileError::NativeModule {
                    details: "native Cranelift subset only supports identifier bindings".to_owned(),
                });
            };

            let value = compile_expr(&mut builder, &bindings, &binding.value)?;
            bindings.insert(name.clone(), value);
            last_value = Some(value);
        }

        let last_value = last_value.ok_or_else(|| CompileError::NativeModule {
            details: "native Cranelift subset requires at least one top-level binding".to_owned(),
        })?;

        let box_ref = module_ctx.declare_func_in_func(box_number, builder.func);
        let boxed_value = builder.ins().call(box_ref, &[last_value]);
        let value_handle = builder.inst_results(boxed_value)[0];
        let print_ref = module_ctx.declare_func_in_func(print_value, builder.func);
        let _ = builder.ins().call(print_ref, &[value_handle]);
        let drop_ref = module_ctx.declare_func_in_func(drop_value, builder.func);
        let _ = builder.ins().call(drop_ref, &[value_handle]);
        let zero = builder.ins().iconst(types::I32, 0);
        builder.ins().return_(&[zero]);
        builder.finalize();
    }

    module_ctx
        .define_function(main_id, &mut context)
        .map_err(|error| CompileError::NativeModule {
            details: format!(
                "failed to define native function for `{}`: {error}",
                object_path
            ),
        })?;
    module_ctx.clear_context(&mut context);
    module_ctx
        .finish()
        .emit()
        .map_err(|error| CompileError::ObjectEmission {
            path: object_path.to_owned(),
            details: error.to_string(),
        })
}

fn declare_value_from_number(
    module: &mut ObjectModule,
) -> Result<cranelift_module::FuncId, CompileError> {
    let mut signature = module.make_signature();
    signature.params.push(AbiParam::new(types::F64));
    signature
        .returns
        .push(AbiParam::new(module.target_config().pointer_type()));

    module
        .declare_function("fscript_value_from_number", Linkage::Import, &signature)
        .map_err(|error| CompileError::NativeModule {
            details: error.to_string(),
        })
}

fn declare_value_print(
    module: &mut ObjectModule,
    pointer_type: ir::Type,
) -> Result<cranelift_module::FuncId, CompileError> {
    let mut signature = module.make_signature();
    signature.params.push(AbiParam::new(pointer_type));

    module
        .declare_function("fscript_value_print", Linkage::Import, &signature)
        .map_err(|error| CompileError::NativeModule {
            details: error.to_string(),
        })
}

fn declare_value_drop(
    module: &mut ObjectModule,
    pointer_type: ir::Type,
) -> Result<cranelift_module::FuncId, CompileError> {
    let mut signature = module.make_signature();
    signature.params.push(AbiParam::new(pointer_type));

    module
        .declare_function("fscript_value_drop", Linkage::Import, &signature)
        .map_err(|error| CompileError::NativeModule {
            details: error.to_string(),
        })
}

fn build_runtime_abi_library(temp_dir: &Utf8Path) -> Result<camino::Utf8PathBuf, CompileError> {
    let cargo_target_dir = temp_dir.join("cargo-target");
    let manifest_path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Utf8Path::parent)
        .expect("crate manifest should live under the workspace root")
        .join("Cargo.toml");
    let cargo_output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--package")
        .arg("fscript-runtime")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .env("CARGO_TARGET_DIR", cargo_target_dir.as_str())
        .output()
        .map_err(|source| CompileError::CargoInvocation { source })?;

    if !cargo_output.status.success() {
        return Err(CompileError::CargoFailed {
            output: cargo_target_dir.clone(),
            stderr: String::from_utf8_lossy(&cargo_output.stderr)
                .trim()
                .to_owned(),
        });
    }

    let library_name = if cfg!(windows) {
        "fscript_runtime.lib"
    } else {
        "libfscript_runtime.a"
    };
    let library_path = cargo_target_dir.join("release").join(library_name);

    if !library_path.exists() {
        return Err(CompileError::CopyCompiledBinary {
            from: library_path.clone(),
            to: temp_dir.to_owned(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "runtime ABI library was not produced by cargo build",
            ),
        });
    }

    Ok(library_path)
}

fn declare_main(module: &mut ObjectModule) -> Result<cranelift_module::FuncId, CompileError> {
    let mut signature = module.make_signature();
    signature.returns.push(AbiParam::new(types::I32));

    module
        .declare_function("main", Linkage::Export, &signature)
        .map_err(|error| CompileError::NativeModule {
            details: error.to_string(),
        })
}

fn item_is_supported(item: &ModuleItem) -> bool {
    let ModuleItem::Binding(binding) = item else {
        return false;
    };

    matches!(binding.pattern, Pattern::Identifier { .. }) && expr_is_supported(&binding.value)
}

fn expr_is_supported(expr: &Expr) -> bool {
    match expr {
        Expr::NumberLiteral { .. } | Expr::Identifier { .. } => true,
        Expr::Unary {
            operator: UnaryOperator::Negate | UnaryOperator::Positive,
            operand,
            ..
        } => expr_is_supported(operand),
        Expr::Binary {
            operator:
                BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide,
            left,
            right,
            ..
        } => expr_is_supported(left) && expr_is_supported(right),
        Expr::Block { items, .. } => {
            !items.is_empty()
                && items.iter().all(|item| match item {
                    BlockItem::Binding(binding) => {
                        matches!(binding.pattern, Pattern::Identifier { .. })
                            && expr_is_supported(&binding.value)
                    }
                    BlockItem::Expr(expression) => expr_is_supported(expression),
                })
        }
        _ => false,
    }
}

fn compile_expr(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<String, ir::Value>,
    expr: &Expr,
) -> Result<ir::Value, CompileError> {
    match expr {
        Expr::NumberLiteral { value, .. } => Ok(builder.ins().f64const(*value)),
        Expr::Identifier { name, .. } => {
            bindings
                .get(name)
                .copied()
                .ok_or_else(|| CompileError::NativeModule {
                    details: format!("native Cranelift subset could not resolve binding `{name}`"),
                })
        }
        Expr::Unary {
            operator: UnaryOperator::Positive,
            operand,
            ..
        } => compile_expr(builder, bindings, operand),
        Expr::Unary {
            operator: UnaryOperator::Negate,
            operand,
            ..
        } => {
            let value = compile_expr(builder, bindings, operand)?;
            Ok(builder.ins().fneg(value))
        }
        Expr::Binary {
            operator,
            left,
            right,
            ..
        } => {
            let left = compile_expr(builder, bindings, left)?;
            let right = compile_expr(builder, bindings, right)?;

            let value = match operator {
                BinaryOperator::Add => builder.ins().fadd(left, right),
                BinaryOperator::Subtract => builder.ins().fsub(left, right),
                BinaryOperator::Multiply => builder.ins().fmul(left, right),
                BinaryOperator::Divide => builder.ins().fdiv(left, right),
                other => {
                    return Err(CompileError::NativeModule {
                        details: format!(
                            "native Cranelift subset does not support binary operator `{other:?}`"
                        ),
                    });
                }
            };

            Ok(value)
        }
        Expr::Block { items, .. } => {
            let mut scoped = bindings.clone();
            let mut last_value = None;

            for item in items {
                match item {
                    BlockItem::Binding(binding) => {
                        let Pattern::Identifier { name, .. } = &binding.pattern else {
                            return Err(CompileError::NativeModule {
                                details:
                                    "native Cranelift subset only supports identifier bindings"
                                        .to_owned(),
                            });
                        };
                        let value = compile_expr(builder, &scoped, &binding.value)?;
                        scoped.insert(name.clone(), value);
                        last_value = Some(value);
                    }
                    BlockItem::Expr(expression) => {
                        last_value = Some(compile_expr(builder, &scoped, expression)?);
                    }
                }
            }

            last_value.ok_or_else(|| CompileError::NativeModule {
                details: "native Cranelift subset requires non-empty block expressions".to_owned(),
            })
        }
        other => Err(CompileError::NativeModule {
            details: format!("native Cranelift subset does not support expression `{other:?}`"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::process::Command;

    use camino::Utf8PathBuf;
    use fscript_ir::{BinaryOperator, BindingDecl, BlockItem, Expr, Module, ModuleItem, Pattern};
    use fscript_source::Span;

    fn span() -> Span {
        Span::new(0, 0)
    }

    fn binding(name: &str, value: Expr) -> ModuleItem {
        ModuleItem::Binding(BindingDecl {
            pattern: Pattern::Identifier {
                name: name.to_owned(),
                span: span(),
            },
            value,
            is_exported: false,
            span: span(),
        })
    }

    #[test]
    fn supports_numeric_single_module_programs() {
        let module = Module {
            items: vec![
                binding(
                    "left",
                    Expr::NumberLiteral {
                        value: 40.0,
                        span: span(),
                    },
                ),
                binding(
                    "answer",
                    Expr::Binary {
                        operator: BinaryOperator::Add,
                        left: Box::new(Expr::Identifier {
                            name: "left".to_owned(),
                            span: span(),
                        }),
                        right: Box::new(Expr::NumberLiteral {
                            value: 2.0,
                            span: span(),
                        }),
                        span: span(),
                    },
                ),
            ],
            exports: vec![],
        };
        let modules = BTreeMap::from([("<entry>".to_owned(), module)]);

        assert!(super::supports_program(&modules, "<entry>"));
    }

    #[test]
    fn rejects_programs_outside_the_first_native_subset() {
        let module = Module {
            items: vec![binding(
                "answer",
                Expr::StringLiteral {
                    value: "hello".to_owned(),
                    span: span(),
                },
            )],
            exports: vec![],
        };
        let modules = BTreeMap::from([("<entry>".to_owned(), module)]);

        assert!(!super::supports_program(&modules, "<entry>"));
    }

    #[test]
    fn rejects_programs_with_missing_entries_or_exports() {
        let module = Module {
            items: vec![binding(
                "value",
                Expr::NumberLiteral {
                    value: 1.0,
                    span: span(),
                },
            )],
            exports: vec!["value".to_owned()],
        };

        assert!(!super::supports_program(&BTreeMap::new(), "<entry>"));
        assert!(!super::supports_program(
            &BTreeMap::from([("<entry>".to_owned(), module)]),
            "<entry>",
        ));
    }

    #[test]
    fn rejects_programs_with_imports_or_multiple_modules() {
        let entry = Module {
            items: vec![ModuleItem::Import(fscript_ir::ImportDecl {
                clause: fscript_ir::ImportClause::Default("dep".to_owned()),
                source: "dep.fs".to_owned(),
                source_span: span(),
                span: span(),
            })],
            exports: vec![],
        };
        let dep = Module {
            items: vec![binding(
                "value",
                Expr::NumberLiteral {
                    value: 1.0,
                    span: span(),
                },
            )],
            exports: vec![],
        };

        assert!(!super::supports_program(
            &BTreeMap::from([
                ("<entry>".to_owned(), entry.clone()),
                ("dep.fs".to_owned(), dep),
            ]),
            "<entry>",
        ));
        assert!(!super::supports_program(
            &BTreeMap::from([("<entry>".to_owned(), entry)]),
            "<entry>",
        ));
    }

    #[test]
    fn emits_and_links_a_native_executable_for_the_numeric_subset() {
        let module = Module {
            items: vec![
                binding(
                    "left",
                    Expr::NumberLiteral {
                        value: 20.0,
                        span: span(),
                    },
                ),
                binding(
                    "answer",
                    Expr::Block {
                        items: vec![
                            BlockItem::Binding(BindingDecl {
                                pattern: Pattern::Identifier {
                                    name: "right".to_owned(),
                                    span: span(),
                                },
                                value: Expr::NumberLiteral {
                                    value: 22.0,
                                    span: span(),
                                },
                                is_exported: false,
                                span: span(),
                            }),
                            BlockItem::Expr(Expr::Binary {
                                operator: BinaryOperator::Add,
                                left: Box::new(Expr::Identifier {
                                    name: "left".to_owned(),
                                    span: span(),
                                }),
                                right: Box::new(Expr::Identifier {
                                    name: "right".to_owned(),
                                    span: span(),
                                }),
                                span: span(),
                            }),
                        ],
                        span: span(),
                    },
                ),
            ],
            exports: vec![],
        };
        let modules = BTreeMap::from([("<entry>".to_owned(), module)]);
        let output = Utf8PathBuf::from_path_buf(
            std::env::temp_dir().join(format!("fscript-native-test-{}", std::process::id())),
        )
        .expect("temp path should be utf-8");

        super::compile_program(&modules, "<entry>", &output)
            .expect("native numeric subset should compile");
        let run = Command::new(&output)
            .output()
            .expect("native executable should run");

        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n");

        let _ = std::fs::remove_file(output);
    }
}
