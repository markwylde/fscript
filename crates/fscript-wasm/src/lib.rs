use fscript_driver::{DiagnosticSummary, check_source, run_source};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, PartialEq, Serialize)]
struct SandboxResult {
    ok: bool,
    token_count: Option<usize>,
    output: Option<String>,
    pretty_error: Option<String>,
    diagnostic: Option<DiagnosticSummary>,
}

#[wasm_bindgen]
pub fn run(source: &str) -> Result<JsValue, JsValue> {
    serialize_result(run_result(source))
}

fn run_result(source: &str) -> SandboxResult {
    match run_source(source) {
        Ok(summary) => SandboxResult {
            ok: true,
            token_count: None,
            output: summary.last_value.map(|value| value.to_string()),
            pretty_error: None,
            diagnostic: None,
        },
        Err(error) => SandboxResult {
            ok: false,
            token_count: None,
            output: None,
            pretty_error: Some(error.render_pretty()),
            diagnostic: Some(error.diagnostic_summary()),
        },
    }
}

#[wasm_bindgen]
pub fn check(source: &str) -> Result<JsValue, JsValue> {
    serialize_result(check_result(source))
}

fn check_result(source: &str) -> SandboxResult {
    match check_source(source) {
        Ok(summary) => SandboxResult {
            ok: true,
            token_count: Some(summary.token_count),
            output: None,
            pretty_error: None,
            diagnostic: None,
        },
        Err(error) => SandboxResult {
            ok: false,
            token_count: None,
            output: None,
            pretty_error: Some(error.render_pretty()),
            diagnostic: Some(error.diagnostic_summary()),
        },
    }
}

fn serialize_result(result: SandboxResult) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&result)
        .map_err(|error| JsValue::from_str(&format!("failed to serialize wasm response: {error}")))
}

#[cfg(test)]
mod tests {
    use super::{SandboxResult, check_result, run_result};

    #[test]
    fn run_result_reports_output_for_valid_programs() {
        let result = run_result("message = 'hello from wasm'");

        assert_eq!(
            result,
            SandboxResult {
                ok: true,
                token_count: None,
                output: Some("hello from wasm".to_owned()),
                pretty_error: None,
                diagnostic: None,
            }
        );
    }

    #[test]
    fn run_result_reports_diagnostics_for_invalid_programs() {
        let result = run_result("message = missing");

        assert!(!result.ok);
        assert_eq!(result.token_count, None);
        assert_eq!(result.output, None);
        assert!(
            result
                .pretty_error
                .as_deref()
                .is_some_and(|error| !error.is_empty())
        );
        let diagnostic = result
            .diagnostic
            .expect("invalid programs should surface a diagnostic summary");
        assert_eq!(diagnostic.kind, "lower");
        assert!(diagnostic.message.contains("missing"));
    }

    #[test]
    fn check_result_reports_token_count_for_valid_programs() {
        let result = check_result("answer = 42");

        assert!(result.ok);
        assert_eq!(result.output, None);
        assert_eq!(result.pretty_error, None);
        assert_eq!(result.diagnostic, None);
        assert_eq!(result.token_count, Some(5));
    }

    #[test]
    fn check_result_reports_diagnostics_for_invalid_programs() {
        let result = check_result("answer 42");

        assert!(!result.ok);
        assert_eq!(result.token_count, None);
        assert_eq!(result.output, None);
        assert!(
            result
                .pretty_error
                .as_deref()
                .is_some_and(|error| !error.is_empty())
        );
        let diagnostic = result
            .diagnostic
            .expect("parse failures should surface a diagnostic summary");
        assert_eq!(diagnostic.kind, "parse");
        assert!(diagnostic.message.contains("expected"));
    }
}
