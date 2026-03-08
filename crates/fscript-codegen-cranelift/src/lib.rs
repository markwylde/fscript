//! Native compilation support for the current executable slice.
//!
//! The repository still uses a mixed backend:
//! - a narrow real Cranelift path handles the first numeric single-module slice
//! - a fixed embedded-runner bridge handles the broader interpreter-backed subset
//!
//! This removes the older generated Rust-source compiler bridge while preserving
//! the broader compile coverage that already exists in the test suite.

mod native;

use std::{
    collections::BTreeMap,
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use camino::{Utf8Path, Utf8PathBuf};
use fscript_ir::{CompiledProgram, Module};
use fscript_runtime::NativeFunction;
use thiserror::Error;

/// Current backend owner for a stdlib export in `fscript compile`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StdlibBackendOwner {
    EmbeddedRunner,
    NativeRuntimeCall,
    NativeLowered,
}

/// Backend status for one stdlib export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StdlibBackendStatus {
    pub module: &'static str,
    pub export: &'static str,
    pub owner: StdlibBackendOwner,
}

const fn stdlib_backend_status(
    function: NativeFunction,
    owner: StdlibBackendOwner,
) -> StdlibBackendStatus {
    StdlibBackendStatus {
        module: function.module_name(),
        export: function.export_name(),
        owner,
    }
}

/// Current stdlib backend parity table for `fscript compile`.
pub const STDLIB_BACKEND_PARITY: [StdlibBackendStatus; NativeFunction::ALL.len()] = [
    stdlib_backend_status(
        NativeFunction::ObjectSpread,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(NativeFunction::ArrayMap, StdlibBackendOwner::EmbeddedRunner),
    stdlib_backend_status(
        NativeFunction::ArrayFilter,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::ArrayLength,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::HttpServe,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::JsonToObject,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::JsonToString,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::JsonToPrettyString,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::LoggerCreate,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::LoggerLog,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::LoggerDebug,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::LoggerInfo,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::LoggerWarn,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::LoggerError,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::LoggerPrettyJson,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::FilesystemReadFile,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::FilesystemWriteFile,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::FilesystemExists,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::FilesystemDeleteFile,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::FilesystemReadDir,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::StringTrim,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::StringUppercase,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::StringLowercase,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::StringIsDigits,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::NumberParse,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(NativeFunction::ResultOk, StdlibBackendOwner::EmbeddedRunner),
    stdlib_backend_status(
        NativeFunction::ResultError,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::ResultIsOk,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::ResultIsError,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::ResultWithDefault,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(NativeFunction::TaskAll, StdlibBackendOwner::EmbeddedRunner),
    stdlib_backend_status(NativeFunction::TaskRace, StdlibBackendOwner::EmbeddedRunner),
    stdlib_backend_status(
        NativeFunction::TaskSpawn,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::TaskDefer,
        StdlibBackendOwner::EmbeddedRunner,
    ),
    stdlib_backend_status(
        NativeFunction::TaskForce,
        StdlibBackendOwner::EmbeddedRunner,
    ),
];

/// Returns the current backend owner for a stdlib export in `fscript compile`.
#[must_use]
pub fn stdlib_backend_owner(module: &str, export: &str) -> Option<StdlibBackendOwner> {
    let mut index = 0;
    while index < STDLIB_BACKEND_PARITY.len() {
        let status = STDLIB_BACKEND_PARITY[index];
        if status.module == module && status.export == export {
            return Some(status.owner);
        }
        index += 1;
    }

    None
}

/// Compiles a single IR module into a native executable.
pub fn compile_module(module: &Module, output: &Utf8Path) -> Result<(), CompileError> {
    let modules = BTreeMap::from([("<entry>".to_owned(), module.clone())]);
    compile_program(&modules, "<entry>", output)
}

/// Compiles the current executable slice into a native executable.
pub fn compile_program(
    modules: &BTreeMap<String, Module>,
    entry: &str,
    output: &Utf8Path,
) -> Result<(), CompileError> {
    if native::supports_program(modules, entry) {
        return native::compile_program(modules, entry, output);
    }

    compile_program_with_embedded_runner(modules, entry, output)
}

fn compile_program_with_embedded_runner(
    modules: &BTreeMap<String, Module>,
    entry: &str,
    output: &Utf8Path,
) -> Result<(), CompileError> {
    if let Some(parent) = output.parent().filter(|parent| !parent.as_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|source| CompileError::CreateOutputDirectory {
            path: parent.to_owned(),
            source,
        })?;
    }

    let temp_dir = create_temp_directory()?;
    let image_path = temp_dir.join("program-image.json");
    let cargo_target_dir = temp_dir.join("cargo-target");
    let image = CompiledProgram {
        entry: entry.to_owned(),
        modules: modules.clone(),
    };
    let encoded = serde_json::to_vec(&image).map_err(|source| CompileError::ProgramImage {
        details: source.to_string(),
    })?;
    fs::write(&image_path, encoded).map_err(|source| CompileError::WriteProgramImage {
        path: image_path.clone(),
        source,
    })?;

    let manifest_path = workspace_root().join("Cargo.toml");
    let cargo_output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--package")
        .arg("fscript-compile-runner")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .env("CARGO_TARGET_DIR", cargo_target_dir.as_str())
        .env("FSCRIPT_PROGRAM_IMAGE_PATH", image_path.as_str())
        .output()
        .map_err(|source| CompileError::CargoInvocation { source })?;

    if !cargo_output.status.success() {
        return Err(CompileError::CargoFailed {
            output: output.to_owned(),
            stderr: String::from_utf8_lossy(&cargo_output.stderr)
                .trim()
                .to_owned(),
        });
    }

    let built_binary = cargo_target_dir.join("release").join(runner_binary_name());

    fs::copy(&built_binary, output).map_err(|source| CompileError::CopyCompiledBinary {
        from: built_binary.clone(),
        to: output.to_owned(),
        source,
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata =
            fs::metadata(&built_binary).map_err(|source| CompileError::CopyCompiledBinary {
                from: built_binary.clone(),
                to: output.to_owned(),
                source,
            })?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(output, permissions).map_err(|source| {
            CompileError::CopyCompiledBinary {
                from: built_binary,
                to: output.to_owned(),
                source,
            }
        })?;
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

fn workspace_root() -> Utf8PathBuf {
    Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Utf8Path::parent)
        .expect("crate manifest should live under the workspace root")
        .to_owned()
}

fn runner_binary_name() -> &'static str {
    if cfg!(windows) {
        "fscript-compile-runner.exe"
    } else {
        "fscript-compile-runner"
    }
}

/// Compilation failures surfaced to the driver.
#[derive(Debug, Error)]
pub enum CompileError {
    #[error("failed to create output directory `{path}`")]
    CreateOutputDirectory {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to create a temporary compilation directory")]
    CreateTempDirectory {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to encode the embedded program image")]
    ProgramImage { details: String },
    #[error("failed to write the embedded program image to `{path}`")]
    WriteProgramImage {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to invoke `cargo`; the embedded runner requires a local Rust toolchain")]
    CargoInvocation {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to configure the native Cranelift target")]
    NativeTargetConfiguration { details: String },
    #[error("failed to declare or define the native Cranelift module")]
    NativeModule { details: String },
    #[error("failed to emit a native object file to `{path}`")]
    ObjectEmission { path: Utf8PathBuf, details: String },
    #[error("failed to invoke a native build tool while building `{output}`")]
    LinkInvocation {
        output: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("a native build tool failed while building `{output}`\n{stderr}")]
    LinkFailed { output: Utf8PathBuf, stderr: String },
    #[error("`cargo build` failed while compiling `{output}`\n{stderr}")]
    CargoFailed { output: Utf8PathBuf, stderr: String },
    #[error("failed to copy compiled binary from `{from}` to `{to}`")]
    CopyCompiledBinary {
        from: Utf8PathBuf,
        to: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl CompileError {
    /// Returns the source span for source-local compilation diagnostics.
    #[must_use]
    pub const fn span(&self) -> Option<fscript_source::Span> {
        match self {
            Self::CreateOutputDirectory { .. }
            | Self::CreateTempDirectory { .. }
            | Self::ProgramImage { .. }
            | Self::WriteProgramImage { .. }
            | Self::CargoInvocation { .. }
            | Self::NativeTargetConfiguration { .. }
            | Self::NativeModule { .. }
            | Self::ObjectEmission { .. }
            | Self::LinkInvocation { .. }
            | Self::LinkFailed { .. }
            | Self::CargoFailed { .. }
            | Self::CopyCompiledBinary { .. } => None,
        }
    }

    /// Returns the module identifier for source-local compilation diagnostics.
    #[must_use]
    pub const fn module(&self) -> Option<&str> {
        None
    }
}

fn create_temp_directory() -> Result<Utf8PathBuf, CompileError> {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "fscript-compile-{}-{unique_suffix}",
        std::process::id()
    ));
    let temp_dir =
        Utf8PathBuf::from_path_buf(temp_dir).expect("temporary directory path should be utf-8");

    fs::create_dir_all(&temp_dir).map_err(|source| CompileError::CreateTempDirectory { source })?;

    Ok(temp_dir)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, collections::BTreeSet, process::Command};

    use camino::Utf8PathBuf;
    use fscript_ir::{BindingDecl, Expr, ModuleItem, Pattern};
    use fscript_runtime::NativeFunction;
    use fscript_source::Span;

    use super::{
        CompileError, STDLIB_BACKEND_PARITY, StdlibBackendOwner, compile_module, compile_program,
        create_temp_directory, runner_binary_name, stdlib_backend_owner, workspace_root,
    };

    fn span() -> Span {
        Span::new(0, 0)
    }

    fn identifier(name: &str) -> Pattern {
        Pattern::Identifier {
            name: name.to_owned(),
            span: span(),
        }
    }

    #[test]
    fn embedded_runner_executes_non_native_programs() {
        let module = fscript_ir::Module {
            items: vec![ModuleItem::Binding(BindingDecl {
                pattern: identifier("message"),
                value: Expr::StringLiteral {
                    value: "hello from runner".to_owned(),
                    span: span(),
                },
                is_exported: false,
                span: span(),
            })],
            exports: vec![],
        };
        let modules = BTreeMap::from([("<entry>".to_owned(), module)]);
        let output_path = temp_binary_path("runner-subset");

        compile_program(&modules, "<entry>", &output_path)
            .expect("embedded-runner subset should compile");

        let output = Command::new(output_path.as_str())
            .output()
            .expect("compiled runner-backed binary should execute");

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "hello from runner\n"
        );

        let _ = std::fs::remove_file(output_path);
    }

    #[test]
    fn compile_helpers_report_platform_and_workspace_metadata() {
        let root = workspace_root();
        assert!(root.join("Cargo.toml").exists());

        if cfg!(windows) {
            assert_eq!(runner_binary_name(), "fscript-compile-runner.exe");
        } else {
            assert_eq!(runner_binary_name(), "fscript-compile-runner");
        }
    }

    #[test]
    fn create_temp_directory_creates_a_real_directory() {
        let temp_dir = create_temp_directory().expect("temp directory should be created");

        assert!(temp_dir.exists());
        assert!(temp_dir.is_dir());

        std::fs::remove_dir_all(&temp_dir).expect("temp directory cleanup should succeed");
    }

    #[test]
    fn compile_error_helpers_return_no_source_metadata() {
        let error = CompileError::ProgramImage {
            details: "broken".to_owned(),
        };

        assert_eq!(error.span(), None);
        assert_eq!(error.module(), None);
    }

    #[test]
    fn compile_module_uses_single_entry_wrapper() {
        let module = fscript_ir::Module {
            items: vec![ModuleItem::Binding(BindingDecl {
                pattern: identifier("value"),
                value: Expr::Record {
                    fields: vec![fscript_ir::RecordField {
                        name: "wrapped".to_owned(),
                        value: Expr::NumberLiteral {
                            value: 1.0,
                            span: span(),
                        },
                        span: span(),
                    }],
                    span: span(),
                },
                is_exported: false,
                span: span(),
            })],
            exports: vec![],
        };
        let output_path = temp_binary_path("compile-module-wrapper");

        compile_module(&module, &output_path)
            .expect("compile_module should compile through the wrapper");

        let output = Command::new(output_path.as_str())
            .output()
            .expect("compiled wrapper-backed binary should execute");

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "{ wrapped: 1 }\n");

        let _ = std::fs::remove_file(output_path);
    }

    #[test]
    fn stdlib_backend_parity_covers_every_runtime_native_export() {
        let expected = NativeFunction::ALL
            .into_iter()
            .map(|function| (function.module_name(), function.export_name()))
            .collect::<BTreeSet<_>>();
        let actual = STDLIB_BACKEND_PARITY
            .iter()
            .map(|status| (status.module, status.export))
            .collect::<BTreeSet<_>>();

        assert_eq!(actual, expected);
    }

    #[test]
    fn stdlib_backend_parity_reports_embedded_runner_ownership_today() {
        for status in STDLIB_BACKEND_PARITY {
            assert_eq!(
                stdlib_backend_owner(status.module, status.export),
                Some(StdlibBackendOwner::EmbeddedRunner)
            );
        }
    }

    fn temp_binary_path(label: &str) -> Utf8PathBuf {
        let temp_dir = create_temp_directory().expect("temporary directory should be created");
        let file_name = if cfg!(windows) {
            format!("{label}.exe")
        } else {
            label.to_owned()
        };

        temp_dir.join(file_name)
    }
}
