use std::{collections::BTreeMap, fs, process::Command};

use camino::Utf8Path;
use cranelift_codegen::{
    ir,
    ir::{AbiParam, InstBuilder, UserFuncName, types},
    settings::{self, Configurable},
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{DataDescription, FuncId, Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};
use fscript_ir::{BinaryOperator, BlockItem, Expr, ImportClause, ModuleItem, Pattern, UnaryOperator};

use crate::CompileError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgramMode {
    Numeric,
    FilesystemExample,
}

pub(crate) fn supports_program(
    modules: &BTreeMap<String, fscript_ir::Module>,
    entry: &str,
) -> bool {
    detect_program_mode(modules, entry).is_some()
}

fn detect_program_mode(
    modules: &BTreeMap<String, fscript_ir::Module>,
    entry: &str,
) -> Option<ProgramMode> {
    if modules.len() != 1 {
        return None;
    }

    let module = modules.get(entry)?;

    if !module.exports.is_empty() {
        return None;
    }

    if module.items.iter().all(item_is_supported) {
        Some(ProgramMode::Numeric)
    } else if module_handle_is_supported(module) {
        Some(ProgramMode::FilesystemExample)
    } else {
        None
    }
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

    let mode = detect_program_mode(modules, entry).ok_or_else(|| CompileError::NativeModule {
        details: "native Cranelift subset does not support this program yet".to_owned(),
    })?;

    let temp_dir = super::create_temp_directory()?;
    let object_path = temp_dir.join("program.o");
    let runtime_library_path = build_runtime_abi_library(&temp_dir)?;

    let emitted = match mode {
        ProgramMode::Numeric => emit_object(module, &object_path)?,
        ProgramMode::FilesystemExample => emit_handle_object(module, &object_path)?,
    };
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StdImportKind {
    Json,
    Filesystem,
}

struct HandleAbi {
    pointer_type: ir::Type,
    value_from_string: FuncId,
    value_from_bool: FuncId,
    value_clone: FuncId,
    value_as_bool: FuncId,
    value_record_new: FuncId,
    value_record_insert: FuncId,
    value_print: FuncId,
    value_drop: FuncId,
    filesystem_write_file: FuncId,
    filesystem_exists: FuncId,
    filesystem_read_file: FuncId,
    filesystem_read_file_defer: FuncId,
    filesystem_delete_file: FuncId,
    json_to_pretty_string: FuncId,
    add: FuncId,
}

fn emit_handle_object(
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
    let abi = declare_handle_abi(&mut module_ctx, pointer_type)?;

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

        let mut imports = BTreeMap::new();
        let mut bindings = BTreeMap::new();
        let mut top_level_values = Vec::new();
        let mut last_value = None;

        for item in &module.items {
            match item {
                ModuleItem::Import(import) => {
                    let Some(kind) = supported_import(import) else {
                        return Err(CompileError::NativeModule {
                            details: format!(
                                "native handle subset does not support import `{}`",
                                import.source
                            ),
                        });
                    };
                    let ImportClause::Default(name) = &import.clause else {
                        return Err(CompileError::NativeModule {
                            details: "native handle subset only supports default std imports"
                                .to_owned(),
                        });
                    };
                    imports.insert(name.clone(), kind);
                }
                ModuleItem::Binding(binding) => {
                    let Pattern::Identifier { name, .. } = &binding.pattern else {
                        return Err(CompileError::NativeModule {
                            details: "native handle subset only supports identifier bindings"
                                .to_owned(),
                        });
                    };

                    let value = compile_handle_expr(
                        &mut module_ctx,
                        &mut builder,
                        &abi,
                        &imports,
                        &bindings,
                        &binding.value,
                    )?;
                    bindings.insert(name.clone(), value);
                    top_level_values.push(value);
                    last_value = Some(value);
                }
            }
        }

        let last_value = last_value.ok_or_else(|| CompileError::NativeModule {
            details: "native handle subset requires at least one top-level binding".to_owned(),
        })?;

        let print_ref = module_ctx.declare_func_in_func(abi.value_print, builder.func);
        let _ = builder.ins().call(print_ref, &[last_value]);

        let drop_ref = module_ctx.declare_func_in_func(abi.value_drop, builder.func);
        for value in top_level_values {
            let _ = builder.ins().call(drop_ref, &[value]);
        }

        let zero = builder.ins().iconst(types::I32, 0);
        builder.ins().return_(&[zero]);
        builder.finalize();
    }

    module_ctx
        .define_function(main_id, &mut context)
        .map_err(|error| CompileError::NativeModule {
            details: format!(
                "failed to define native handle function for `{}`: {error}",
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

fn supported_import(import: &fscript_ir::ImportDecl) -> Option<StdImportKind> {
    match (&import.clause, import.source.as_str()) {
        (ImportClause::Default(_), "std:json") => Some(StdImportKind::Json),
        (ImportClause::Default(_), "std:filesystem") => Some(StdImportKind::Filesystem),
        _ => None,
    }
}

fn module_handle_is_supported(module: &fscript_ir::Module) -> bool {
    let mut imports = BTreeMap::new();
    for item in &module.items {
        match item {
            ModuleItem::Import(import) => {
                let Some(kind) = supported_import(import) else {
                    return false;
                };
                let ImportClause::Default(name) = &import.clause else {
                    return false;
                };
                imports.insert(name.clone(), kind);
            }
            ModuleItem::Binding(binding) => {
                if !matches!(binding.pattern, Pattern::Identifier { .. })
                    || !handle_expr_is_supported(&imports, &binding.value)
                {
                    return false;
                }
            }
        }
    }

    true
}

fn handle_expr_is_supported(imports: &BTreeMap<String, StdImportKind>, expr: &Expr) -> bool {
    match expr {
        Expr::StringLiteral { .. } | Expr::BooleanLiteral { .. } | Expr::Identifier { .. } => true,
        Expr::Record { fields, .. } => fields
            .iter()
            .all(|field| handle_expr_is_supported(imports, &field.value)),
        Expr::Block { items, .. } => !items.is_empty()
            && items.iter().all(|item| match item {
                BlockItem::Binding(binding) => {
                    matches!(binding.pattern, Pattern::Identifier { .. })
                        && handle_expr_is_supported(imports, &binding.value)
                }
                BlockItem::Expr(expr) => handle_expr_is_supported(imports, expr),
            }),
        Expr::If {
            condition,
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => {
            handle_expr_is_supported(imports, condition)
                && handle_expr_is_supported(imports, then_branch)
                && handle_expr_is_supported(imports, else_branch)
        }
        Expr::Unary {
            operator: UnaryOperator::Defer,
            operand,
            ..
        } => matches!(
            operand.as_ref(),
            Expr::Call { callee, args, .. }
                if args.len() == 1
                    && matches!(
                        callee.as_ref(),
                        Expr::Member { object, property, .. }
                            if property == "readFile"
                                && matches!(
                                    object.as_ref(),
                                    Expr::Identifier { name, .. }
                                        if imports
                                            .get(name)
                                            .is_some_and(|kind| *kind == StdImportKind::Filesystem)
                                )
                    )
                    && handle_expr_is_supported(imports, &args[0])
        ),
        Expr::Binary {
            operator: BinaryOperator::Add,
            left,
            right,
            ..
        } => handle_expr_is_supported(imports, left) && handle_expr_is_supported(imports, right),
        Expr::Call { callee, args, .. } => {
            args.iter().all(|arg| handle_expr_is_supported(imports, arg))
                && matches!(
                    callee.as_ref(),
                    Expr::Member { object, property, .. }
                        if matches!(
                            object.as_ref(),
                            Expr::Identifier { name, .. }
                                if matches!(
                                    (imports.get(name), property.as_str()),
                                    (Some(StdImportKind::Json), "jsonToPrettyString")
                                        | (Some(StdImportKind::Filesystem), "writeFile")
                                        | (Some(StdImportKind::Filesystem), "exists")
                                        | (Some(StdImportKind::Filesystem), "readFile")
                                        | (Some(StdImportKind::Filesystem), "deleteFile")
                                )
                        )
                )
        }
        _ => false,
    }
}

fn compile_handle_expr(
    module_ctx: &mut ObjectModule,
    builder: &mut FunctionBuilder<'_>,
    abi: &HandleAbi,
    imports: &BTreeMap<String, StdImportKind>,
    bindings: &BTreeMap<String, ir::Value>,
    expr: &Expr,
) -> Result<ir::Value, CompileError> {
    match expr {
        Expr::StringLiteral { value, .. } => compile_string_handle(module_ctx, builder, abi, value),
        Expr::BooleanLiteral { value, .. } => {
            let value = builder.ins().iconst(types::I8, i64::from(*value));
            let func = module_ctx.declare_func_in_func(abi.value_from_bool, builder.func);
            let call = builder.ins().call(func, &[value]);
            Ok(builder.inst_results(call)[0])
        }
        Expr::Identifier { name, .. } => {
            let Some(value) = bindings.get(name).copied() else {
                return Err(CompileError::NativeModule {
                    details: format!("native handle subset could not resolve binding `{name}`"),
                });
            };
            let func = module_ctx.declare_func_in_func(abi.value_clone, builder.func);
            let call = builder.ins().call(func, &[value]);
            Ok(builder.inst_results(call)[0])
        }
        Expr::Record { fields, .. } => {
            let func = module_ctx.declare_func_in_func(abi.value_record_new, builder.func);
            let record_call = builder.ins().call(func, &[]);
            let mut record = builder.inst_results(record_call)[0];
            for field in fields {
                let field_value =
                    compile_handle_expr(module_ctx, builder, abi, imports, bindings, &field.value)?;
                let (key_ptr, key_len) =
                    compile_bytes_reference(module_ctx, builder, abi.pointer_type, field.name.as_bytes())?;
                let insert = module_ctx.declare_func_in_func(abi.value_record_insert, builder.func);
                let call = builder
                    .ins()
                    .call(insert, &[record, key_ptr, key_len, field_value]);
                record = builder.inst_results(call)[0];
            }
            Ok(record)
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
                                    "native handle subset only supports identifier bindings"
                                        .to_owned(),
                            });
                        };
                        let value = compile_handle_expr(
                            module_ctx,
                            builder,
                            abi,
                            imports,
                            &scoped,
                            &binding.value,
                        )?;
                        scoped.insert(name.clone(), value);
                        last_value = Some(value);
                    }
                    BlockItem::Expr(expr) => {
                        last_value =
                            Some(compile_handle_expr(module_ctx, builder, abi, imports, &scoped, expr)?);
                    }
                }
            }
            last_value.ok_or_else(|| CompileError::NativeModule {
                details: "native handle subset requires non-empty block expressions".to_owned(),
            })
        }
        Expr::If {
            condition,
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => compile_handle_if(
            module_ctx,
            builder,
            abi,
            imports,
            bindings,
            condition,
            (then_branch, else_branch),
        ),
        Expr::Unary {
            operator: UnaryOperator::Defer,
            operand,
            ..
        } => compile_handle_defer(module_ctx, builder, abi, imports, bindings, operand),
        Expr::Binary {
            operator: BinaryOperator::Add,
            left,
            right,
            ..
        } => {
            let left = compile_handle_expr(module_ctx, builder, abi, imports, bindings, left)?;
            let right = compile_handle_expr(module_ctx, builder, abi, imports, bindings, right)?;
            let func = module_ctx.declare_func_in_func(abi.add, builder.func);
            let call = builder.ins().call(func, &[left, right]);
            Ok(builder.inst_results(call)[0])
        }
        Expr::Call { callee, args, .. } => {
            compile_handle_std_call(module_ctx, builder, abi, imports, bindings, callee, args)
        }
        other => Err(CompileError::NativeModule {
            details: format!("native handle subset does not support expression `{other:?}`"),
        }),
    }
}

fn compile_handle_if(
    module_ctx: &mut ObjectModule,
    builder: &mut FunctionBuilder<'_>,
    abi: &HandleAbi,
    imports: &BTreeMap<String, StdImportKind>,
    bindings: &BTreeMap<String, ir::Value>,
    condition: &Expr,
    branches: (&Expr, &Expr),
) -> Result<ir::Value, CompileError> {
    let (then_branch, else_branch) = branches;
    let condition_value = compile_handle_expr(module_ctx, builder, abi, imports, bindings, condition)?;
    let as_bool = module_ctx.declare_func_in_func(abi.value_as_bool, builder.func);
    let call = builder.ins().call(as_bool, &[condition_value]);
    let bool_value = builder.inst_results(call)[0];
    let is_true = builder.ins().icmp_imm(ir::condcodes::IntCC::NotEqual, bool_value, 0);

    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, abi.pointer_type);

    builder.ins().brif(is_true, then_block, &[], else_block, &[]);
    builder.seal_block(then_block);
    builder.seal_block(else_block);

    builder.switch_to_block(then_block);
    let then_value = compile_handle_expr(module_ctx, builder, abi, imports, bindings, then_branch)?;
    let then_args = [ir::BlockArg::Value(then_value)];
    builder.ins().jump(merge_block, &then_args);

    builder.switch_to_block(else_block);
    let else_value = compile_handle_expr(module_ctx, builder, abi, imports, bindings, else_branch)?;
    let else_args = [ir::BlockArg::Value(else_value)];
    builder.ins().jump(merge_block, &else_args);

    builder.seal_block(merge_block);
    builder.switch_to_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}

fn compile_handle_defer(
    module_ctx: &mut ObjectModule,
    builder: &mut FunctionBuilder<'_>,
    abi: &HandleAbi,
    imports: &BTreeMap<String, StdImportKind>,
    bindings: &BTreeMap<String, ir::Value>,
    operand: &Expr,
) -> Result<ir::Value, CompileError> {
    let Expr::Call { callee, args, .. } = operand else {
        return Err(CompileError::NativeModule {
            details: "native handle subset only supports `defer` on direct std calls".to_owned(),
        });
    };
    let Expr::Member { object, property, .. } = callee.as_ref() else {
        return Err(CompileError::NativeModule {
            details: "native handle subset only supports `defer` on std member calls".to_owned(),
        });
    };
    let Expr::Identifier { name, .. } = object.as_ref() else {
        return Err(CompileError::NativeModule {
            details: "native handle subset only supports imported std modules in `defer`"
                .to_owned(),
        });
    };
    if imports.get(name) != Some(&StdImportKind::Filesystem) || property != "readFile" || args.len() != 1 {
        return Err(CompileError::NativeModule {
            details: "native handle subset only supports `defer FileSystem.readFile(path)`"
                .to_owned(),
        });
    }

    let path = compile_handle_expr(module_ctx, builder, abi, imports, bindings, &args[0])?;
    let func = module_ctx.declare_func_in_func(abi.filesystem_read_file_defer, builder.func);
    let call = builder.ins().call(func, &[path]);
    Ok(builder.inst_results(call)[0])
}

fn compile_handle_std_call(
    module_ctx: &mut ObjectModule,
    builder: &mut FunctionBuilder<'_>,
    abi: &HandleAbi,
    imports: &BTreeMap<String, StdImportKind>,
    bindings: &BTreeMap<String, ir::Value>,
    callee: &Expr,
    args: &[Expr],
) -> Result<ir::Value, CompileError> {
    let Expr::Member { object, property, .. } = callee else {
        return Err(CompileError::NativeModule {
            details: "native handle subset only supports direct std member calls".to_owned(),
        });
    };
    let Expr::Identifier { name, .. } = object.as_ref() else {
        return Err(CompileError::NativeModule {
            details: "native handle subset only supports imported std modules in calls".to_owned(),
        });
    };
    let Some(kind) = imports.get(name) else {
        return Err(CompileError::NativeModule {
            details: format!("native handle subset does not know imported module `{name}`"),
        });
    };

    let arg_values = args
        .iter()
        .map(|arg| compile_handle_expr(module_ctx, builder, abi, imports, bindings, arg))
        .collect::<Result<Vec<_>, _>>()?;

    let func_id = match (kind, property.as_str(), arg_values.len()) {
        (StdImportKind::Json, "jsonToPrettyString", 1) => abi.json_to_pretty_string,
        (StdImportKind::Filesystem, "writeFile", 2) => abi.filesystem_write_file,
        (StdImportKind::Filesystem, "exists", 1) => abi.filesystem_exists,
        (StdImportKind::Filesystem, "readFile", 1) => abi.filesystem_read_file,
        (StdImportKind::Filesystem, "deleteFile", 1) => abi.filesystem_delete_file,
        _ => {
            return Err(CompileError::NativeModule {
                details: format!(
                    "native handle subset does not support `{name}.{property}` with {} argument(s)",
                    arg_values.len()
                ),
            });
        }
    };
    let func = module_ctx.declare_func_in_func(func_id, builder.func);
    let call = builder.ins().call(func, &arg_values);
    Ok(builder.inst_results(call)[0])
}

fn compile_string_handle(
    module_ctx: &mut ObjectModule,
    builder: &mut FunctionBuilder<'_>,
    abi: &HandleAbi,
    value: &str,
) -> Result<ir::Value, CompileError> {
    let (ptr, len) = compile_bytes_reference(module_ctx, builder, abi.pointer_type, value.as_bytes())?;
    let func = module_ctx.declare_func_in_func(abi.value_from_string, builder.func);
    let call = builder.ins().call(func, &[ptr, len]);
    Ok(builder.inst_results(call)[0])
}

fn compile_bytes_reference(
    module_ctx: &mut ObjectModule,
    builder: &mut FunctionBuilder<'_>,
    pointer_type: ir::Type,
    bytes: &[u8],
) -> Result<(ir::Value, ir::Value), CompileError> {
    let data_id = module_ctx
        .declare_anonymous_data(false, false)
        .map_err(|error| CompileError::NativeModule {
            details: error.to_string(),
        })?;
    let mut data = DataDescription::new();
    data.define(bytes.to_vec().into_boxed_slice());
    module_ctx
        .define_data(data_id, &data)
        .map_err(|error| CompileError::NativeModule {
            details: error.to_string(),
        })?;
    let global = module_ctx.declare_data_in_func(data_id, builder.func);
    let ptr = builder.ins().global_value(pointer_type, global);
    let len = builder.ins().iconst(pointer_type, bytes.len() as i64);
    Ok((ptr, len))
}

fn declare_handle_abi(
    module: &mut ObjectModule,
    pointer_type: ir::Type,
) -> Result<HandleAbi, CompileError> {
    Ok(HandleAbi {
        pointer_type,
        value_from_string: declare_handle_fn(
            module,
            "fscript_value_from_string",
            &[pointer_type, pointer_type],
            Some(pointer_type),
        )?,
        value_from_bool: declare_handle_fn(
            module,
            "fscript_value_from_bool",
            &[types::I8],
            Some(pointer_type),
        )?,
        value_clone: declare_handle_fn(module, "fscript_value_clone", &[pointer_type], Some(pointer_type))?,
        value_as_bool: declare_handle_fn(module, "fscript_value_as_bool", &[pointer_type], Some(types::I8))?,
        value_record_new: declare_handle_fn(module, "fscript_value_record_new", &[], Some(pointer_type))?,
        value_record_insert: declare_handle_fn(
            module,
            "fscript_value_record_insert",
            &[pointer_type, pointer_type, pointer_type, pointer_type],
            Some(pointer_type),
        )?,
        value_print: declare_handle_fn(module, "fscript_value_print", &[pointer_type], None)?,
        value_drop: declare_handle_fn(module, "fscript_value_drop", &[pointer_type], None)?,
        filesystem_write_file: declare_handle_fn(
            module,
            "fscript_std_filesystem_write_file",
            &[pointer_type, pointer_type],
            Some(pointer_type),
        )?,
        filesystem_exists: declare_handle_fn(
            module,
            "fscript_std_filesystem_exists",
            &[pointer_type],
            Some(pointer_type),
        )?,
        filesystem_read_file: declare_handle_fn(
            module,
            "fscript_std_filesystem_read_file",
            &[pointer_type],
            Some(pointer_type),
        )?,
        filesystem_read_file_defer: declare_handle_fn(
            module,
            "fscript_std_filesystem_read_file_defer",
            &[pointer_type],
            Some(pointer_type),
        )?,
        filesystem_delete_file: declare_handle_fn(
            module,
            "fscript_std_filesystem_delete_file",
            &[pointer_type],
            Some(pointer_type),
        )?,
        json_to_pretty_string: declare_handle_fn(
            module,
            "fscript_std_json_to_pretty_string",
            &[pointer_type],
            Some(pointer_type),
        )?,
        add: declare_handle_fn(
            module,
            "fscript_std_add",
            &[pointer_type, pointer_type],
            Some(pointer_type),
        )?,
    })
}

fn declare_handle_fn(
    module: &mut ObjectModule,
    name: &str,
    params: &[ir::Type],
    ret: Option<ir::Type>,
) -> Result<FuncId, CompileError> {
    let mut signature = module.make_signature();
    for param in params {
        signature.params.push(AbiParam::new(*param));
    }
    if let Some(ret) = ret {
        signature.returns.push(AbiParam::new(ret));
    }

    module
        .declare_function(name, Linkage::Import, &signature)
        .map_err(|error| CompileError::NativeModule {
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
        .arg("fscript-std")
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
        "fscript_std.lib"
    } else {
        "libfscript_std.a"
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
    use fscript_ir::{
        BinaryOperator, BindingDecl, BlockItem, Expr, Module, ModuleItem, Pattern, UnaryOperator,
    };
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
    fn supports_the_handle_based_filesystem_slice() {
        let module = Module {
            items: vec![
                ModuleItem::Import(fscript_ir::ImportDecl {
                    clause: fscript_ir::ImportClause::Default("Json".to_owned()),
                    source: "std:json".to_owned(),
                    source_span: span(),
                    span: span(),
                }),
                ModuleItem::Import(fscript_ir::ImportDecl {
                    clause: fscript_ir::ImportClause::Default("FileSystem".to_owned()),
                    source: "std:filesystem".to_owned(),
                    source_span: span(),
                    span: span(),
                }),
                binding(
                    "path",
                    Expr::StringLiteral {
                        value: "/tmp/demo.txt".to_owned(),
                        span: span(),
                    },
                ),
                binding(
                    "reader",
                    Expr::Unary {
                        operator: UnaryOperator::Defer,
                        operand: Box::new(Expr::Call {
                            callee: Box::new(Expr::Member {
                                object: Box::new(Expr::Identifier {
                                    name: "FileSystem".to_owned(),
                                    span: span(),
                                }),
                                property: "readFile".to_owned(),
                                span: span(),
                            }),
                            args: vec![Expr::Identifier {
                                name: "path".to_owned(),
                                span: span(),
                            }],
                            span: span(),
                        }),
                        span: span(),
                    },
                ),
                binding(
                    "answer",
                    Expr::Call {
                        callee: Box::new(Expr::Member {
                            object: Box::new(Expr::Identifier {
                                name: "Json".to_owned(),
                                span: span(),
                            }),
                            property: "jsonToPrettyString".to_owned(),
                            span: span(),
                        }),
                        args: vec![Expr::Record {
                            fields: vec![fscript_ir::RecordField {
                                name: "contents".to_owned(),
                                value: Expr::Binary {
                                    operator: BinaryOperator::Add,
                                    left: Box::new(Expr::Identifier {
                                        name: "reader".to_owned(),
                                        span: span(),
                                    }),
                                    right: Box::new(Expr::StringLiteral {
                                        value: String::new(),
                                        span: span(),
                                    }),
                                    span: span(),
                                },
                                span: span(),
                            }],
                            span: span(),
                        }],
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
