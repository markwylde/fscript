use fscript_ir::CompiledProgram;

fn main() {
    let image_path = env!("FSCRIPT_PROGRAM_IMAGE_PATH");
    let image_bytes = include_bytes!(env!("FSCRIPT_PROGRAM_IMAGE_PATH"));
    let image = match decode_program_image(image_path, image_bytes) {
        Ok(image) => image,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    match execute_program_image(&image) {
        Ok(Some(value)) => println!("{value}"),
        Ok(None) => {}
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn decode_program_image(image_path: &str, image_bytes: &[u8]) -> Result<CompiledProgram, String> {
    serde_json::from_slice(image_bytes).map_err(|error| {
        format!("failed to decode embedded FScript program image from `{image_path}`: {error}")
    })
}

fn execute_program_image(image: &CompiledProgram) -> Result<Option<String>, String> {
    fscript_interpreter::run_program(&image.modules, &image.entry)
        .map(|value| value.map(|value| value.to_string()))
        .map_err(|error| error.message().to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use fscript_ir::{BindingDecl, CompiledProgram, Expr, Module, ModuleItem, Pattern};
    use fscript_source::Span;

    use super::{decode_program_image, execute_program_image};

    fn span() -> Span {
        Span::new(0, 0)
    }

    fn identifier(name: &str) -> Pattern {
        Pattern::Identifier {
            name: name.to_owned(),
            span: span(),
        }
    }

    fn compiled_program_with_expr(expr: Expr) -> CompiledProgram {
        CompiledProgram {
            entry: "<entry>".to_owned(),
            modules: BTreeMap::from([(
                "<entry>".to_owned(),
                Module {
                    items: vec![ModuleItem::Binding(BindingDecl {
                        pattern: identifier("value"),
                        value: expr,
                        is_exported: false,
                        span: span(),
                    })],
                    exports: vec![],
                },
            )]),
        }
    }

    #[test]
    fn decodes_serialized_program_images() {
        let image = compiled_program_with_expr(Expr::NumberLiteral {
            value: 42.0,
            span: span(),
        });
        let encoded = serde_json::to_vec(&image).expect("image should serialize");

        let decoded = decode_program_image("program-image.json", &encoded)
            .expect("valid image should decode");

        assert_eq!(decoded, image);
    }

    #[test]
    fn reports_invalid_program_images() {
        let error = decode_program_image("broken.json", br#"{"entry":true}"#)
            .expect_err("invalid image should fail");

        assert!(error.contains("failed to decode embedded FScript program image"));
        assert!(error.contains("broken.json"));
    }

    #[test]
    fn executes_serialized_program_images() {
        let image = compiled_program_with_expr(Expr::StringLiteral {
            value: "runner output".to_owned(),
            span: span(),
        });

        let value = execute_program_image(&image).expect("program should execute");

        assert_eq!(value, Some("runner output".to_owned()));
    }

    #[test]
    fn reports_runtime_failures() {
        let image = compiled_program_with_expr(Expr::Identifier {
            name: "missing".to_owned(),
            span: span(),
        });

        let error = execute_program_image(&image).expect_err("runtime failure should surface");

        assert!(error.contains("unknown identifier `missing`"));
    }

    #[test]
    fn execute_program_image_returns_none_for_empty_modules() {
        let image = CompiledProgram {
            entry: "<entry>".to_owned(),
            modules: BTreeMap::from([(
                "<entry>".to_owned(),
                Module {
                    items: Vec::new(),
                    exports: Vec::new(),
                },
            )]),
        };

        let value = execute_program_image(&image).expect("empty modules should execute");

        assert_eq!(value, None);
    }
}
