//! Runtime-backed `std:` module bindings for the first executable slice.

use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write, stderr, stdout},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    str::Chars,
    time::Duration,
};

use fscript_runtime::{
    DeferredBody, DeferredValue, NativeFunction, NativeFunctionValue, RuntimeError, Value,
};

/// Loads a runtime-backed `std:` module into a record of exported values.
pub fn load_module(source: &str) -> Result<Value, RuntimeError> {
    match source {
        "std:array" => Ok(native_module(&[
            ("map", NativeFunction::ArrayMap),
            ("filter", NativeFunction::ArrayFilter),
            ("length", NativeFunction::ArrayLength),
        ])),
        "std:json" => Ok(native_module(&[
            ("parse", NativeFunction::JsonToObject),
            ("stringify", NativeFunction::JsonToString),
            ("jsonToObject", NativeFunction::JsonToObject),
            ("jsonToString", NativeFunction::JsonToString),
            ("jsonToPrettyString", NativeFunction::JsonToPrettyString),
        ])),
        "std:logger" => Ok(native_module(&[
            ("create", NativeFunction::LoggerCreate),
            ("log", NativeFunction::LoggerLog),
            ("debug", NativeFunction::LoggerDebug),
            ("info", NativeFunction::LoggerInfo),
            ("warn", NativeFunction::LoggerWarn),
            ("error", NativeFunction::LoggerError),
            ("prettyJson", NativeFunction::LoggerPrettyJson),
        ])),
        "std:http" => Ok(native_module(&[("serve", NativeFunction::HttpServe)])),
        "std:filesystem" => Ok(native_module(&[
            ("readFile", NativeFunction::FilesystemReadFile),
            ("writeFile", NativeFunction::FilesystemWriteFile),
            ("exists", NativeFunction::FilesystemExists),
            ("deleteFile", NativeFunction::FilesystemDeleteFile),
            ("readDir", NativeFunction::FilesystemReadDir),
        ])),
        "std:object" => Ok(native_module(&[("spread", NativeFunction::ObjectSpread)])),
        "std:string" => Ok(native_module(&[
            ("trim", NativeFunction::StringTrim),
            ("uppercase", NativeFunction::StringUppercase),
            ("lowercase", NativeFunction::StringLowercase),
            ("isDigits", NativeFunction::StringIsDigits),
        ])),
        "std:number" => Ok(native_module(&[("parse", NativeFunction::NumberParse)])),
        "std:result" => Ok(native_module(&[
            ("ok", NativeFunction::ResultOk),
            ("error", NativeFunction::ResultError),
            ("isOk", NativeFunction::ResultIsOk),
            ("isError", NativeFunction::ResultIsError),
            ("withDefault", NativeFunction::ResultWithDefault),
        ])),
        "std:task" => Ok(native_module(&[
            ("all", NativeFunction::TaskAll),
            ("race", NativeFunction::TaskRace),
            ("spawn", NativeFunction::TaskSpawn),
            ("defer", NativeFunction::TaskDefer),
            ("force", NativeFunction::TaskForce),
        ])),
        _ => Err(RuntimeError::new(format!(
            "unknown standard library module `{source}`"
        ))),
    }
}

/// Executes a host-native stdlib function.
pub fn execute_native_function<F, G, E>(
    function: NativeFunction,
    args: Vec<Value>,
    mut call: F,
    mut force: G,
) -> Result<Value, E>
where
    F: FnMut(Value, Vec<Value>) -> Result<Value, E>,
    G: FnMut(Value) -> Result<Value, E>,
    E: From<RuntimeError>,
{
    match function {
        NativeFunction::ObjectSpread => {
            let [left, right]: [Value; 2] = args.try_into().map_err(|_| {
                E::from(RuntimeError::new(
                    "Object.spread expected exactly 2 arguments",
                ))
            })?;
            spread_records(left, right).map_err(E::from)
        }
        NativeFunction::ArrayMap => array_map(args, &mut call),
        NativeFunction::ArrayFilter => array_filter(args, &mut call),
        NativeFunction::ArrayLength => array_length(args).map_err(E::from),
        NativeFunction::HttpServe => http_serve(args, &mut call),
        NativeFunction::JsonToObject => json_to_object(args).map_err(E::from),
        NativeFunction::JsonToString => json_to_string(args).map_err(E::from),
        NativeFunction::JsonToPrettyString => json_to_pretty_string(args).map_err(E::from),
        NativeFunction::LoggerCreate => logger_create(args).map_err(E::from),
        NativeFunction::LoggerLog => {
            logger_log(args, LoggerSeverity::Info, "Logger.log").map_err(E::from)
        }
        NativeFunction::LoggerDebug => {
            logger_log(args, LoggerSeverity::Debug, "Logger.debug").map_err(E::from)
        }
        NativeFunction::LoggerInfo => {
            logger_log(args, LoggerSeverity::Info, "Logger.info").map_err(E::from)
        }
        NativeFunction::LoggerWarn => {
            logger_log(args, LoggerSeverity::Warn, "Logger.warn").map_err(E::from)
        }
        NativeFunction::LoggerError => {
            logger_log(args, LoggerSeverity::Error, "Logger.error").map_err(E::from)
        }
        NativeFunction::LoggerPrettyJson => logger_pretty_json(args).map_err(E::from),
        NativeFunction::FilesystemReadFile => filesystem_read_file(args).map_err(E::from),
        NativeFunction::FilesystemWriteFile => filesystem_write_file(args).map_err(E::from),
        NativeFunction::FilesystemExists => filesystem_exists(args).map_err(E::from),
        NativeFunction::FilesystemDeleteFile => filesystem_delete_file(args).map_err(E::from),
        NativeFunction::FilesystemReadDir => filesystem_read_dir(args).map_err(E::from),
        NativeFunction::StringTrim => {
            map_string_value(args, |value| value.trim().to_owned()).map_err(E::from)
        }
        NativeFunction::StringUppercase => {
            map_string_value(args, |value| value.to_uppercase()).map_err(E::from)
        }
        NativeFunction::StringLowercase => {
            map_string_value(args, |value| value.to_lowercase()).map_err(E::from)
        }
        NativeFunction::StringIsDigits => string_is_digits(args).map_err(E::from),
        NativeFunction::NumberParse => number_parse(args).map_err(E::from),
        NativeFunction::ResultOk => tagged_result("ok", "value", args).map_err(E::from),
        NativeFunction::ResultError => tagged_result("error", "error", args).map_err(E::from),
        NativeFunction::ResultIsOk => result_has_tag("ok", args).map_err(E::from),
        NativeFunction::ResultIsError => result_has_tag("error", args).map_err(E::from),
        NativeFunction::ResultWithDefault => result_with_default(args).map_err(E::from),
        NativeFunction::TaskAll => task_all(args).map_err(E::from),
        NativeFunction::TaskRace => task_race(args).map_err(E::from),
        NativeFunction::TaskSpawn => task_spawn(args, &mut call, &mut force),
        NativeFunction::TaskDefer => task_defer(args).map_err(E::from),
        NativeFunction::TaskForce => task_force(args, &mut force),
    }
}

fn native_module(exports: &[(&str, NativeFunction)]) -> Value {
    Value::Record(BTreeMap::from_iter(exports.iter().map(
        |(name, function)| {
            (
                (*name).to_owned(),
                Value::NativeFunction(NativeFunctionValue::new(*function)),
            )
        },
    )))
}

fn spread_records(left: Value, right: Value) -> Result<Value, RuntimeError> {
    let Value::Record(left) = left else {
        return Err(RuntimeError::new(
            "Object.spread expects record values for its left argument",
        ));
    };
    let Value::Record(right) = right else {
        return Err(RuntimeError::new(
            "Object.spread expects record values for its right argument",
        ));
    };

    let mut merged = left;
    for (key, value) in right {
        merged.insert(key, value);
    }

    Ok(Value::Record(merged))
}

fn array_map<F, E>(args: Vec<Value>, call: &mut F) -> Result<Value, E>
where
    F: FnMut(Value, Vec<Value>) -> Result<Value, E>,
    E: From<RuntimeError>,
{
    let [function, items]: [Value; 2] = args
        .try_into()
        .map_err(|_| E::from(RuntimeError::new("Array.map expected exactly 2 arguments")))?;
    let Value::Array(items) = items else {
        return Err(E::from(RuntimeError::new(
            "Array.map expects an array as its final argument",
        )));
    };

    let mapped = items
        .into_iter()
        .map(|item| call(function.clone(), vec![item]))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Value::Array(mapped))
}

fn array_filter<F, E>(args: Vec<Value>, call: &mut F) -> Result<Value, E>
where
    F: FnMut(Value, Vec<Value>) -> Result<Value, E>,
    E: From<RuntimeError>,
{
    let [function, items]: [Value; 2] = args.try_into().map_err(|_| {
        E::from(RuntimeError::new(
            "Array.filter expected exactly 2 arguments",
        ))
    })?;
    let Value::Array(items) = items else {
        return Err(E::from(RuntimeError::new(
            "Array.filter expects an array as its final argument",
        )));
    };

    let mut filtered = Vec::new();
    for item in items {
        match call(function.clone(), vec![item.clone()])? {
            Value::Boolean(true) => filtered.push(item),
            Value::Boolean(false) => {}
            other => {
                return Err(E::from(RuntimeError::new(format!(
                    "Array.filter callbacks must return Boolean values, found `{other}`"
                ))));
            }
        }
    }

    Ok(Value::Array(filtered))
}

fn array_length(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [items]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Array.length expected exactly 1 argument"))?;

    match items {
        Value::Array(items) | Value::Sequence(items) => Ok(Value::Number(items.len() as f64)),
        other => Err(RuntimeError::new(format!(
            "Array.length expects an array value, found `{other}`"
        ))),
    }
}

fn http_serve<F, E>(args: Vec<Value>, call: &mut F) -> Result<Value, E>
where
    F: FnMut(Value, Vec<Value>) -> Result<Value, E>,
    E: From<RuntimeError>,
{
    let [options, handler]: [Value; 2] = args
        .try_into()
        .map_err(|_| E::from(RuntimeError::new("Http.serve expected exactly 2 arguments")))?;
    let options = parse_http_options(options).map_err(E::from)?;

    let listener = TcpListener::bind((options.host.as_str(), options.port)).map_err(|error| {
        E::from(RuntimeError::new(format!(
            "Http.serve failed to bind {}:{}: {error}",
            options.host, options.port
        )))
    })?;

    let mut served_requests = 0_usize;
    loop {
        if matches!(options.max_requests, Some(limit) if served_requests >= limit) {
            break;
        }

        let (mut stream, address) = listener.accept().map_err(|error| {
            E::from(RuntimeError::new(format!(
                "Http.serve failed to accept a connection: {error}"
            )))
        })?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

        let request = read_http_request(&mut stream).map_err(E::from)?;
        let request_value = request_to_value(&request);
        let response = call(handler.clone(), vec![request_value])?;
        let response = parse_http_response(response).map_err(E::from)?;
        write_http_response(&mut stream, &response).map_err(E::from)?;
        served_requests += 1;

        eprintln!(
            "fscript http served {} {} from {} ({}/{})",
            request.method,
            request.path,
            address,
            served_requests,
            options.max_requests.unwrap_or(0)
        );
    }

    Ok(Value::Undefined)
}

fn json_to_object(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [text]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Json.jsonToObject expected exactly 1 argument"))?;
    let Value::String(text) = text else {
        return Err(RuntimeError::new(
            "Json.jsonToObject expects a String argument",
        ));
    };
    let text = strip_relaxed_json_comments(&text)?;

    let mut parser = JsonParser::new(&text);
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.peek().is_some() {
        return Err(RuntimeError::new(
            "Json.jsonToObject found trailing content after the first JSON value",
        ));
    }

    Ok(value)
}

#[derive(Clone, Debug)]
struct HttpServeOptions {
    host: String,
    port: u16,
    max_requests: Option<usize>,
}

#[derive(Clone, Debug)]
struct HttpRequest {
    method: String,
    path: String,
    body: String,
}

#[derive(Clone, Debug)]
struct HttpResponse {
    status: u16,
    content_type: String,
    body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum LoggerSeverity {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoggerDestination {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LoggerConfig {
    name: Option<String>,
    level: LoggerSeverity,
    destination: LoggerDestination,
}

impl LoggerSeverity {
    fn api_name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

fn strip_relaxed_json_comments(text: &str) -> Result<String, RuntimeError> {
    let mut stripped = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if in_string {
            stripped.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            stripped.push(ch);
            continue;
        }

        if ch == '/' {
            match chars.peek().copied() {
                Some('/') => {
                    chars.next();
                    for comment_char in chars.by_ref() {
                        if comment_char == '\n' {
                            stripped.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    chars.next();
                    let mut prev = None;
                    let mut closed = false;
                    for comment_char in chars.by_ref() {
                        if comment_char == '\n' {
                            stripped.push('\n');
                        }
                        if prev == Some('*') && comment_char == '/' {
                            closed = true;
                            break;
                        }
                        prev = Some(comment_char);
                    }
                    if !closed {
                        return Err(RuntimeError::new(
                            "Json.jsonToObject found an unterminated block comment",
                        ));
                    }
                    continue;
                }
                _ => {}
            }
        }

        if ch == '#' {
            for comment_char in chars.by_ref() {
                if comment_char == '\n' {
                    stripped.push('\n');
                    break;
                }
            }
            continue;
        }

        stripped.push(ch);
    }

    Ok(stripped
        .lines()
        .filter(|line| line.trim() != "---")
        .collect::<Vec<_>>()
        .join("\n"))
}

fn parse_logger_config(value: &Value, context: &str) -> Result<LoggerConfig, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::new(format!(
            "{context} must be a Logger record"
        )));
    };

    let name = match fields.get("name") {
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Null) | None => None,
        Some(other) => {
            return Err(RuntimeError::new(format!(
                "{context}.name must be String or Null, found `{other}`"
            )));
        }
    };
    let level = match record_string_field(fields, "level", context)?.as_str() {
        "debug" => LoggerSeverity::Debug,
        "info" => LoggerSeverity::Info,
        "warn" => LoggerSeverity::Warn,
        "error" => LoggerSeverity::Error,
        other => {
            return Err(RuntimeError::new(format!(
                "{context}.level found unsupported level `{other}`"
            )));
        }
    };
    let destination = match record_string_field(fields, "destination", context)?.as_str() {
        "stdout" => LoggerDestination::Stdout,
        "stderr" => LoggerDestination::Stderr,
        other => {
            return Err(RuntimeError::new(format!(
                "{context}.destination found unsupported destination `{other}`"
            )));
        }
    };

    Ok(LoggerConfig {
        name,
        level,
        destination,
    })
}

fn logger_to_value(config: &LoggerConfig) -> Value {
    Value::Record(BTreeMap::from([
        (
            "name".to_owned(),
            config.name.clone().map_or(Value::Null, Value::String),
        ),
        (
            "level".to_owned(),
            Value::String(config.level.api_name().to_owned()),
        ),
        (
            "destination".to_owned(),
            Value::String(
                match config.destination {
                    LoggerDestination::Stdout => "stdout",
                    LoggerDestination::Stderr => "stderr",
                }
                .to_owned(),
            ),
        ),
    ]))
}

fn format_logger_line(config: &LoggerConfig, severity: LoggerSeverity, message: &str) -> String {
    let mut line = String::new();
    if let Some(name) = &config.name {
        line.push('[');
        line.push_str(name);
        line.push_str("] ");
    }
    line.push_str(severity.label());
    line.push(' ');
    line.push_str(message);
    line.push('\n');
    line
}

fn write_logger_output(destination: LoggerDestination, line: &str) -> Result<(), RuntimeError> {
    match destination {
        LoggerDestination::Stdout => {
            let mut handle = stdout();
            handle
                .write_all(line.as_bytes())
                .map_err(|error| RuntimeError::new(format!("Logger output failed: {error}")))
        }
        LoggerDestination::Stderr => {
            let mut handle = stderr();
            handle
                .write_all(line.as_bytes())
                .map_err(|error| RuntimeError::new(format!("Logger output failed: {error}")))
        }
    }
}

fn parse_http_options(value: Value) -> Result<HttpServeOptions, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::new(
            "Http.serve expects an options record as its first argument",
        ));
    };

    let host = record_string_field(&fields, "host", "Http.serve options")?;
    let port = record_number_field(&fields, "port", "Http.serve options")?;
    let max_requests = record_number_field(&fields, "maxRequests", "Http.serve options")?;

    if !(0.0..=(u16::MAX as f64)).contains(&port) || port.fract() != 0.0 {
        return Err(RuntimeError::new(
            "Http.serve options.port must be an integer between 0 and 65535",
        ));
    }

    if max_requests < 0.0 || max_requests.fract() != 0.0 {
        return Err(RuntimeError::new(
            "Http.serve options.maxRequests must be a non-negative integer",
        ));
    }

    Ok(HttpServeOptions {
        host,
        port: port as u16,
        max_requests: if max_requests == 0.0 {
            None
        } else {
            Some(max_requests as usize)
        },
    })
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, RuntimeError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];

    loop {
        let read = stream.read(&mut buffer).map_err(|error| {
            RuntimeError::new(format!(
                "Http.serve failed while reading a request: {error}"
            ))
        })?;
        if read == 0 {
            break;
        }

        bytes.extend_from_slice(&buffer[..read]);
        if find_bytes(&bytes, b"\r\n\r\n").is_some() {
            break;
        }

        if bytes.len() > 64 * 1024 {
            return Err(RuntimeError::new(
                "Http.serve request headers exceeded the 64KB limit",
            ));
        }
    }

    let Some(header_end) = find_bytes(&bytes, b"\r\n\r\n") else {
        return Err(RuntimeError::new(
            "Http.serve received an incomplete HTTP request",
        ));
    };

    let header_text = String::from_utf8(bytes[..header_end].to_vec())
        .map_err(|_| RuntimeError::new("Http.serve request headers must be valid UTF-8"))?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| RuntimeError::new("Http.serve request is missing a request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| RuntimeError::new("Http.serve request line is missing a method"))?;
    let path = parts
        .next()
        .ok_or_else(|| RuntimeError::new("Http.serve request line is missing a path"))?;
    let _version = parts
        .next()
        .ok_or_else(|| RuntimeError::new("Http.serve request line is missing an HTTP version"))?;

    let content_length = lines
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);

    let body_start = header_end + 4;
    while bytes.len().saturating_sub(body_start) < content_length {
        let read = stream.read(&mut buffer).map_err(|error| {
            RuntimeError::new(format!(
                "Http.serve failed while reading a request body: {error}"
            ))
        })?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }

    let body_end = body_start + content_length.min(bytes.len().saturating_sub(body_start));
    let body = String::from_utf8(bytes[body_start..body_end].to_vec())
        .map_err(|_| RuntimeError::new("Http.serve request body must be valid UTF-8"))?;

    Ok(HttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        body,
    })
}

fn request_to_value(request: &HttpRequest) -> Value {
    Value::Record(BTreeMap::from([
        ("body".to_owned(), Value::String(request.body.clone())),
        ("method".to_owned(), Value::String(request.method.clone())),
        ("path".to_owned(), Value::String(request.path.clone())),
    ]))
}

fn parse_http_response(value: Value) -> Result<HttpResponse, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::new(
            "Http.serve handlers must return a response record",
        ));
    };

    let status = record_number_field(&fields, "status", "Http.serve response")?;
    if !(100.0..=599.0).contains(&status) || status.fract() != 0.0 {
        return Err(RuntimeError::new(
            "Http.serve response.status must be an integer between 100 and 599",
        ));
    }

    Ok(HttpResponse {
        status: status as u16,
        content_type: record_string_field(&fields, "contentType", "Http.serve response")?,
        body: record_string_field(&fields, "body", "Http.serve response")?,
    })
}

fn write_http_response(
    stream: &mut TcpStream,
    response: &HttpResponse,
) -> Result<(), RuntimeError> {
    let body = response.body.as_bytes();
    let headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        reason_phrase(response.status),
        response.content_type,
        body.len()
    );

    stream
        .write_all(headers.as_bytes())
        .and_then(|()| stream.write_all(body))
        .map_err(|error| {
            RuntimeError::new(format!(
                "Http.serve failed while writing a response: {error}"
            ))
        })
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn json_to_string(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [value]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Json.jsonToString expected exactly 1 argument"))?;

    Ok(Value::String(stringify_json_value(
        &value,
        JsonFormat::Compact,
    )?))
}

fn json_to_pretty_string(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [value]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Json.jsonToPrettyString expected exactly 1 argument"))?;

    Ok(Value::String(stringify_json_value(
        &value,
        JsonFormat::Pretty,
    )?))
}

fn logger_create(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [options]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Logger.create expected exactly 1 argument"))?;
    let logger = parse_logger_config(&options, "Logger.create options")?;
    Ok(logger_to_value(&logger))
}

fn logger_log(
    args: Vec<Value>,
    severity: LoggerSeverity,
    function_name: &str,
) -> Result<Value, RuntimeError> {
    let [logger, message]: [Value; 2] = args
        .try_into()
        .map_err(|_| RuntimeError::new(format!("{function_name} expected exactly 2 arguments")))?;
    let logger = parse_logger_config(&logger, "Logger logger")?;
    let Value::String(message) = message else {
        return Err(RuntimeError::new(format!(
            "{function_name} expects a String message argument"
        )));
    };

    if severity >= logger.level {
        let line = format_logger_line(&logger, severity, &message);
        write_logger_output(logger.destination, &line)?;
    }

    Ok(Value::Undefined)
}

fn logger_pretty_json(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [logger, value]: [Value; 2] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Logger.prettyJson expected exactly 2 arguments"))?;
    let logger = parse_logger_config(&logger, "Logger logger")?;

    if LoggerSeverity::Info >= logger.level {
        let line = format_logger_line(
            &logger,
            LoggerSeverity::Info,
            &stringify_json_value(&value, JsonFormat::Pretty)?,
        );
        write_logger_output(logger.destination, &line)?;
    }

    Ok(Value::Undefined)
}

fn filesystem_read_file(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [path]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("FileSystem.readFile expected exactly 1 argument"))?;
    let path = filesystem_path(path, "FileSystem.readFile")?;

    fs::read_to_string(&path)
        .map(Value::String)
        .map_err(|error| filesystem_error("FileSystem.readFile", &path, error))
}

fn filesystem_write_file(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [path, content]: [Value; 2] = args
        .try_into()
        .map_err(|_| RuntimeError::new("FileSystem.writeFile expected exactly 2 arguments"))?;
    let path = filesystem_path(path, "FileSystem.writeFile")?;
    let Value::String(content) = content else {
        return Err(RuntimeError::new(
            "FileSystem.writeFile expects a String content argument",
        ));
    };

    fs::write(&path, content)
        .map_err(|error| filesystem_error("FileSystem.writeFile", &path, error))?;
    Ok(Value::Undefined)
}

fn filesystem_exists(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [path]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("FileSystem.exists expected exactly 1 argument"))?;
    let path = filesystem_path(path, "FileSystem.exists")?;

    Ok(Value::Boolean(path.exists()))
}

fn filesystem_delete_file(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [path]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("FileSystem.deleteFile expected exactly 1 argument"))?;
    let path = filesystem_path(path, "FileSystem.deleteFile")?;

    fs::remove_file(&path)
        .map_err(|error| filesystem_error("FileSystem.deleteFile", &path, error))?;
    Ok(Value::Undefined)
}

fn filesystem_read_dir(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [path]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("FileSystem.readDir expected exactly 1 argument"))?;
    let path = filesystem_path(path, "FileSystem.readDir")?;

    let mut entries = fs::read_dir(&path)
        .map_err(|error| filesystem_error("FileSystem.readDir", &path, error))?
        .map(|entry| {
            let entry =
                entry.map_err(|error| filesystem_error("FileSystem.readDir", &path, error))?;
            let name = entry.file_name();
            let name = name.into_string().map_err(|name| {
                RuntimeError::new(format!(
                    "FileSystem.readDir encountered a non-utf8 filename in `{}`: {:?}",
                    path.display(),
                    name
                ))
            })?;
            Ok(Value::String(name))
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    entries.sort_by(|left, right| match (left, right) {
        (Value::String(left), Value::String(right)) => left.cmp(right),
        _ => std::cmp::Ordering::Equal,
    });

    Ok(Value::Array(entries))
}

fn map_string_value(
    args: Vec<Value>,
    map: impl FnOnce(&str) -> String,
) -> Result<Value, RuntimeError> {
    let [value]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("string helpers expect exactly 1 argument"))?;
    let Value::String(value) = value else {
        return Err(RuntimeError::new("string helpers expect a String argument"));
    };

    Ok(Value::String(map(&value)))
}

fn string_is_digits(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [value]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("String.isDigits expected exactly 1 argument"))?;
    let Value::String(value) = value else {
        return Err(RuntimeError::new(
            "String.isDigits expects a String argument",
        ));
    };

    Ok(Value::Boolean(
        !value.is_empty() && value.chars().all(|character| character.is_ascii_digit()),
    ))
}

fn number_parse(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [value]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Number.parse expected exactly 1 argument"))?;
    let Value::String(value) = value else {
        return Err(RuntimeError::new("Number.parse expects a String argument"));
    };

    value
        .parse::<f64>()
        .map(Value::Number)
        .map_err(|_| RuntimeError::new(format!("could not parse `{value}` as a Number")))
}

fn tagged_result(tag: &str, field: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [value]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Result constructors expect exactly 1 argument"))?;

    Ok(Value::Record(BTreeMap::from([
        ("tag".to_owned(), Value::String(tag.to_owned())),
        (field.to_owned(), value),
    ])))
}

fn result_has_tag(expected_tag: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [result]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Result predicates expect exactly 1 argument"))?;
    let Value::Record(fields) = result else {
        return Err(RuntimeError::new(
            "Result predicates expect a Result record",
        ));
    };

    Ok(Value::Boolean(
        fields.get("tag") == Some(&Value::String(expected_tag.to_owned())),
    ))
}

fn result_with_default(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [fallback, result]: [Value; 2] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Result.withDefault expected exactly 2 arguments"))?;
    let Value::Record(fields) = result else {
        return Err(RuntimeError::new(
            "Result.withDefault expects a Result record",
        ));
    };

    match fields.get("tag") {
        Some(Value::String(tag)) if tag == "ok" => fields
            .get("value")
            .cloned()
            .ok_or_else(|| RuntimeError::new("ok results must contain a `value` field")),
        Some(Value::String(tag)) if tag == "error" => Ok(fallback),
        _ => Err(RuntimeError::new(
            "Result.withDefault expects a tagged Result record",
        )),
    }
}

fn task_all(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [tasks]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Task.all expected exactly 1 argument"))?;
    let Value::Array(tasks) = tasks else {
        return Err(RuntimeError::new(
            "Task.all expects an array of task values",
        ));
    };

    for task in &tasks {
        ensure_task_value(task, "Task.all")?;
    }

    Ok(Value::Deferred(DeferredValue::new(DeferredBody::Batch(
        tasks,
    ))))
}

fn task_race(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [tasks]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Task.race expected exactly 1 argument"))?;
    let Value::Array(tasks) = tasks else {
        return Err(RuntimeError::new(
            "Task.race expects an array of task values",
        ));
    };

    if tasks.is_empty() {
        return Err(RuntimeError::new(
            "Task.race expects a non-empty array of tasks",
        ));
    }

    for task in &tasks {
        ensure_task_value(task, "Task.race")?;
    }

    Ok(Value::Deferred(DeferredValue::new(DeferredBody::Race(
        tasks,
    ))))
}

fn task_spawn<F, G, E>(args: Vec<Value>, _call: &mut F, force: &mut G) -> Result<Value, E>
where
    F: FnMut(Value, Vec<Value>) -> Result<Value, E>,
    G: FnMut(Value) -> Result<Value, E>,
    E: From<RuntimeError>,
{
    let [task]: [Value; 1] = args
        .try_into()
        .map_err(|_| E::from(RuntimeError::new("Task.spawn expected exactly 1 argument")))?;
    ensure_task_value(&task, "Task.spawn").map_err(E::from)?;

    let deferred = match task {
        Value::Deferred(deferred) => deferred,
        other => DeferredValue::new(DeferredBody::Call(Box::new(other))),
    };
    force(Value::Deferred(deferred.clone()))?;
    Ok(Value::Deferred(deferred))
}

fn task_defer(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let [task]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeError::new("Task.defer expected exactly 1 argument"))?;
    ensure_zero_arg_callable(&task, "Task.defer")?;

    Ok(Value::Deferred(DeferredValue::new(DeferredBody::Call(
        Box::new(task),
    ))))
}

fn task_force<G, E>(args: Vec<Value>, force: &mut G) -> Result<Value, E>
where
    G: FnMut(Value) -> Result<Value, E>,
    E: From<RuntimeError>,
{
    let [value]: [Value; 1] = args
        .try_into()
        .map_err(|_| E::from(RuntimeError::new("Task.force expected exactly 1 argument")))?;
    force(value)
}

fn ensure_task_value(value: &Value, function_name: &str) -> Result<(), RuntimeError> {
    match value {
        Value::Deferred(_) => Ok(()),
        _ => ensure_zero_arg_callable(value, function_name),
    }
}

fn ensure_zero_arg_callable(value: &Value, function_name: &str) -> Result<(), RuntimeError> {
    match value {
        Value::Function(function) => {
            let remaining = function.arity().saturating_sub(function.applied_args.len());
            if remaining == 0 {
                Ok(())
            } else {
                Err(RuntimeError::new(format!(
                    "{function_name} expects zero-argument callables, found a function that still needs {remaining} argument(s)"
                )))
            }
        }
        Value::NativeFunction(function) => {
            let remaining = function
                .function
                .arity()
                .saturating_sub(function.applied_args.len());
            if remaining == 0 {
                Ok(())
            } else {
                Err(RuntimeError::new(format!(
                    "{function_name} expects zero-argument callables, found a function that still needs {remaining} argument(s)"
                )))
            }
        }
        other => Err(RuntimeError::new(format!(
            "{function_name} expects zero-argument callables, found `{other}`"
        ))),
    }
}

fn record_string_field(
    fields: &BTreeMap<String, Value>,
    field: &str,
    context: &str,
) -> Result<String, RuntimeError> {
    match fields.get(field) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(other) => Err(RuntimeError::new(format!(
            "{context}.{field} must be a String, found `{other}`"
        ))),
        None => Err(RuntimeError::new(format!(
            "{context} is missing the `{field}` field"
        ))),
    }
}

fn record_number_field(
    fields: &BTreeMap<String, Value>,
    field: &str,
    context: &str,
) -> Result<f64, RuntimeError> {
    match fields.get(field) {
        Some(Value::Number(value)) => Ok(*value),
        Some(other) => Err(RuntimeError::new(format!(
            "{context}.{field} must be a Number, found `{other}`"
        ))),
        None => Err(RuntimeError::new(format!(
            "{context} is missing the `{field}` field"
        ))),
    }
}

fn filesystem_path(value: Value, function_name: &str) -> Result<PathBuf, RuntimeError> {
    let Value::String(path) = value else {
        return Err(RuntimeError::new(format!(
            "{function_name} expects a String path argument"
        )));
    };

    Ok(PathBuf::from(path))
}

fn filesystem_error(function_name: &str, path: &Path, error: std::io::Error) -> RuntimeError {
    RuntimeError::new(format!(
        "{function_name} failed for `{}`: {error}",
        path.display()
    ))
}

#[derive(Clone, Copy)]
enum JsonFormat {
    Compact,
    Pretty,
}

fn stringify_json_value(value: &Value, format: JsonFormat) -> Result<String, RuntimeError> {
    stringify_json_value_with_depth(value, format, 0)
}

fn stringify_json_value_with_depth(
    value: &Value,
    format: JsonFormat,
    depth: usize,
) -> Result<String, RuntimeError> {
    match value {
        Value::String(value) => Ok(format!("\"{}\"", escape_json_string(value))),
        Value::Number(value) => {
            if !value.is_finite() {
                return Err(RuntimeError::new(
                    "Json.jsonToString cannot encode non-finite Number values",
                ));
            }
            Ok(value.to_string())
        }
        Value::Boolean(value) => Ok(value.to_string()),
        Value::Null => Ok("null".to_owned()),
        Value::Array(items) | Value::Sequence(items) => stringify_json_items(items, format, depth),
        Value::Record(fields) => stringify_json_record(fields, format, depth),
        Value::Undefined => Err(RuntimeError::new(
            "Json.jsonToString cannot encode Undefined values",
        )),
        Value::Deferred(_) => Err(RuntimeError::new(
            "Json.jsonToString cannot encode deferred values",
        )),
        Value::Function(_) | Value::NativeFunction(_) => Err(RuntimeError::new(
            "Json.jsonToString cannot encode function values",
        )),
    }
}

fn stringify_json_items(
    items: &[Value],
    format: JsonFormat,
    depth: usize,
) -> Result<String, RuntimeError> {
    if items.is_empty() {
        return Ok("[]".to_owned());
    }

    match format {
        JsonFormat::Compact => {
            let items = items
                .iter()
                .map(|item| stringify_json_value_with_depth(item, format, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", items.join(",")))
        }
        JsonFormat::Pretty => {
            let indent = "  ".repeat(depth + 1);
            let closing_indent = "  ".repeat(depth);
            let items = items
                .iter()
                .map(|item| {
                    Ok(format!(
                        "{}{}",
                        indent,
                        stringify_json_value_with_depth(item, format, depth + 1)?
                    ))
                })
                .collect::<Result<Vec<_>, RuntimeError>>()?;
            Ok(format!("[\n{}\n{}]", items.join(",\n"), closing_indent))
        }
    }
}

fn stringify_json_record(
    fields: &BTreeMap<String, Value>,
    format: JsonFormat,
    depth: usize,
) -> Result<String, RuntimeError> {
    if fields.is_empty() {
        return Ok("{}".to_owned());
    }

    match format {
        JsonFormat::Compact => {
            let fields = fields
                .iter()
                .map(|(name, value)| {
                    Ok(format!(
                        "\"{}\":{}",
                        escape_json_string(name),
                        stringify_json_value_with_depth(value, format, depth + 1)?
                    ))
                })
                .collect::<Result<Vec<_>, RuntimeError>>()?;
            Ok(format!("{{{}}}", fields.join(",")))
        }
        JsonFormat::Pretty => {
            let indent = "  ".repeat(depth + 1);
            let closing_indent = "  ".repeat(depth);
            let fields = fields
                .iter()
                .map(|(name, value)| {
                    Ok(format!(
                        "{}\"{}\": {}",
                        indent,
                        escape_json_string(name),
                        stringify_json_value_with_depth(value, format, depth + 1)?
                    ))
                })
                .collect::<Result<Vec<_>, RuntimeError>>()?;
            Ok(format!("{{\n{}\n{}}}", fields.join(",\n"), closing_indent))
        }
    }
}

fn escape_json_string(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            other => escaped.push(other),
        }
    }
    escaped
}

struct JsonParser<'a> {
    chars: Chars<'a>,
    current: Option<char>,
}

impl<'a> JsonParser<'a> {
    fn new(source: &'a str) -> Self {
        let mut chars = source.chars();
        let current = chars.next();
        Self { chars, current }
    }

    fn parse_value(&mut self) -> Result<Value, RuntimeError> {
        self.skip_whitespace();
        match self.peek() {
            Some('"') => self.parse_string().map(Value::String),
            Some('0'..='9' | '-') => self.parse_number().map(Value::Number),
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('t') => {
                self.expect_keyword("true")?;
                Ok(Value::Boolean(true))
            }
            Some('f') => {
                self.expect_keyword("false")?;
                Ok(Value::Boolean(false))
            }
            Some('n') => {
                self.expect_keyword("null")?;
                Ok(Value::Null)
            }
            Some(other) => Err(RuntimeError::new(format!(
                "Json.jsonToObject found unexpected character `{other}`"
            ))),
            None => Err(RuntimeError::new("Json.jsonToObject expected a JSON value")),
        }
    }

    fn parse_object(&mut self) -> Result<Value, RuntimeError> {
        self.expect_char('{')?;
        self.skip_whitespace();

        let mut fields = BTreeMap::new();
        if self.peek() == Some('}') {
            self.advance();
            return Ok(Value::Record(fields));
        }

        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect_char(':')?;
            let value = self.parse_value()?;
            fields.insert(key, value);
            self.skip_whitespace();

            match self.peek() {
                Some(',') => {
                    self.advance();
                }
                Some('}') => {
                    self.advance();
                    return Ok(Value::Record(fields));
                }
                Some(other) => {
                    return Err(RuntimeError::new(format!(
                        "Json.jsonToObject expected `,` or `}}`, found `{other}`"
                    )));
                }
                None => {
                    return Err(RuntimeError::new(
                        "Json.jsonToObject reached the end of input inside an object",
                    ));
                }
            }
        }
    }

    fn parse_array(&mut self) -> Result<Value, RuntimeError> {
        self.expect_char('[')?;
        self.skip_whitespace();

        let mut items = Vec::new();
        if self.peek() == Some(']') {
            self.advance();
            return Ok(Value::Array(items));
        }

        loop {
            items.push(self.parse_value()?);
            self.skip_whitespace();

            match self.peek() {
                Some(',') => {
                    self.advance();
                }
                Some(']') => {
                    self.advance();
                    return Ok(Value::Array(items));
                }
                Some(other) => {
                    return Err(RuntimeError::new(format!(
                        "Json.jsonToObject expected `,` or `]`, found `{other}`"
                    )));
                }
                None => {
                    return Err(RuntimeError::new(
                        "Json.jsonToObject reached the end of input inside an array",
                    ));
                }
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, RuntimeError> {
        self.expect_char('"')?;
        let mut value = String::new();

        loop {
            match self.peek() {
                Some('"') => {
                    self.advance();
                    return Ok(value);
                }
                Some('\\') => {
                    self.advance();
                    value.push(self.parse_escape_sequence()?);
                }
                Some(ch) => {
                    if ch.is_control() {
                        return Err(RuntimeError::new(
                            "Json.jsonToObject strings cannot contain control characters",
                        ));
                    }
                    value.push(ch);
                    self.advance();
                }
                None => {
                    return Err(RuntimeError::new(
                        "Json.jsonToObject reached the end of input inside a string",
                    ));
                }
            }
        }
    }

    fn parse_escape_sequence(&mut self) -> Result<char, RuntimeError> {
        let Some(escape) = self.peek() else {
            return Err(RuntimeError::new(
                "Json.jsonToObject found an unterminated escape sequence",
            ));
        };
        self.advance();

        match escape {
            '"' => Ok('"'),
            '\\' => Ok('\\'),
            '/' => Ok('/'),
            'b' => Ok('\u{08}'),
            'f' => Ok('\u{0c}'),
            'n' => Ok('\n'),
            'r' => Ok('\r'),
            't' => Ok('\t'),
            'u' => self.parse_unicode_escape(),
            other => Err(RuntimeError::new(format!(
                "Json.jsonToObject found an unsupported escape sequence `\\{other}`"
            ))),
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, RuntimeError> {
        let mut codepoint = 0u32;
        for _ in 0..4 {
            let Some(ch) = self.peek() else {
                return Err(RuntimeError::new(
                    "Json.jsonToObject found an incomplete unicode escape",
                ));
            };
            self.advance();
            let digit = ch.to_digit(16).ok_or_else(|| {
                RuntimeError::new(format!(
                    "Json.jsonToObject found an invalid unicode escape digit `{ch}`"
                ))
            })?;
            codepoint = (codepoint << 4) | digit;
        }

        char::from_u32(codepoint).ok_or_else(|| {
            RuntimeError::new("Json.jsonToObject found an invalid unicode scalar value")
        })
    }

    fn parse_number(&mut self) -> Result<f64, RuntimeError> {
        let mut number = String::new();

        if self.peek() == Some('-') {
            number.push('-');
            self.advance();
        }

        match self.peek() {
            Some('0') => {
                number.push('0');
                self.advance();
            }
            Some('1'..='9') => {
                while let Some(ch @ '0'..='9') = self.peek() {
                    number.push(ch);
                    self.advance();
                }
            }
            _ => {
                return Err(RuntimeError::new(
                    "Json.jsonToObject expected a valid number",
                ));
            }
        }

        if self.peek() == Some('.') {
            number.push('.');
            self.advance();

            let mut digits = 0;
            while let Some(ch @ '0'..='9') = self.peek() {
                number.push(ch);
                self.advance();
                digits += 1;
            }

            if digits == 0 {
                return Err(RuntimeError::new(
                    "Json.jsonToObject expected digits after the decimal point",
                ));
            }
        }

        if matches!(self.peek(), Some('e' | 'E')) {
            number.push('e');
            self.advance();

            if let Some(sign @ ('+' | '-')) = self.peek() {
                number.push(sign);
                self.advance();
            }

            let mut digits = 0;
            while let Some(ch @ '0'..='9') = self.peek() {
                number.push(ch);
                self.advance();
                digits += 1;
            }

            if digits == 0 {
                return Err(RuntimeError::new(
                    "Json.jsonToObject expected exponent digits after `e`",
                ));
            }
        }

        number
            .parse::<f64>()
            .map_err(|_| RuntimeError::new(format!("Json.jsonToObject could not parse `{number}`")))
    }

    fn expect_keyword(&mut self, expected: &str) -> Result<(), RuntimeError> {
        for ch in expected.chars() {
            self.expect_char(ch)?;
        }
        Ok(())
    }

    fn expect_char(&mut self, expected: char) -> Result<(), RuntimeError> {
        match self.peek() {
            Some(actual) if actual == expected => {
                self.advance();
                Ok(())
            }
            Some(actual) => Err(RuntimeError::new(format!(
                "Json.jsonToObject expected `{expected}`, found `{actual}`"
            ))),
            None => Err(RuntimeError::new(format!(
                "Json.jsonToObject expected `{expected}`, found the end of input"
            ))),
        }
    }

    fn skip_whitespace(&mut self) {
        loop {
            while matches!(self.peek(), Some(ch) if ch.is_whitespace()) {
                self.advance();
            }

            if self.peek() != Some('/') {
                return;
            }

            self.advance();
            match self.peek() {
                Some('/') => {
                    while let Some(ch) = self.peek() {
                        self.advance();
                        if ch == '\n' {
                            break;
                        }
                    }
                }
                Some('*') => {
                    self.advance();
                    let mut prev = None;
                    loop {
                        let Some(ch) = self.peek() else {
                            return;
                        };
                        self.advance();
                        if prev == Some('*') && ch == '/' {
                            break;
                        }
                        prev = Some(ch);
                    }
                }
                _ => {
                    self.current = Some('/');
                    return;
                }
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.current
    }

    fn advance(&mut self) {
        self.current = self.chars.next();
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, path::PathBuf};

    use super::{
        JsonFormat, LoggerConfig, LoggerDestination, LoggerSeverity, execute_native_function,
        format_logger_line, load_module, logger_to_value, parse_http_options, parse_http_response,
        parse_logger_config, read_http_request, stringify_json_value,
    };
    use fscript_ir as ir;
    use fscript_runtime::{
        DeferredBody, DeferredValue, NativeFunction, NativeFunctionValue, RuntimeError, Value,
    };
    use fscript_source::Span;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "fscript-std-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after the unix epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn loads_std_array_module() {
        let module = load_module("std:array").expect("std:array should load");
        let Value::Record(exports) = module else {
            panic!("stdlib modules should be records");
        };

        assert!(exports.contains_key("map"));
        assert!(exports.contains_key("length"));
    }

    #[test]
    fn loads_std_task_module() {
        let module = load_module("std:task").expect("std:task should load");
        let Value::Record(exports) = module else {
            panic!("stdlib modules should be records");
        };

        assert!(exports.contains_key("all"));
        assert!(exports.contains_key("race"));
        assert!(exports.contains_key("spawn"));
        assert!(exports.contains_key("defer"));
        assert!(exports.contains_key("force"));
    }

    #[test]
    fn rejects_unknown_std_module() {
        assert_eq!(
            load_module("std:missing"),
            Err(RuntimeError::new(
                "unknown standard library module `std:missing`"
            ))
        );
    }

    #[test]
    fn loads_std_json_module() {
        let module = load_module("std:json").expect("std:json should load");
        let Value::Record(exports) = module else {
            panic!("stdlib modules should be records");
        };

        assert!(exports.contains_key("parse"));
        assert!(exports.contains_key("stringify"));
        assert!(exports.contains_key("jsonToObject"));
        assert!(exports.contains_key("jsonToString"));
        assert!(exports.contains_key("jsonToPrettyString"));
    }

    #[test]
    fn loads_std_logger_module() {
        let module = load_module("std:logger").expect("std:logger should load");
        let Value::Record(exports) = module else {
            panic!("stdlib modules should be records");
        };

        assert!(exports.contains_key("create"));
        assert!(exports.contains_key("log"));
        assert!(exports.contains_key("debug"));
        assert!(exports.contains_key("info"));
        assert!(exports.contains_key("warn"));
        assert!(exports.contains_key("error"));
        assert!(exports.contains_key("prettyJson"));
    }

    #[test]
    fn loads_std_filesystem_module() {
        let module = load_module("std:filesystem").expect("std:filesystem should load");
        let Value::Record(exports) = module else {
            panic!("stdlib modules should be records");
        };

        assert!(exports.contains_key("readFile"));
        assert!(exports.contains_key("writeFile"));
        assert!(exports.contains_key("exists"));
        assert!(exports.contains_key("deleteFile"));
        assert!(exports.contains_key("readDir"));
    }

    #[test]
    fn array_map_calls_back_into_the_interpreter() {
        let result = execute_native_function(
            NativeFunction::ArrayMap,
            vec![
                Value::String("callable".to_owned()),
                Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]),
            ],
            |callee, args| -> Result<Value, RuntimeError> {
                assert_eq!(callee, Value::String("callable".to_owned()));
                let [Value::Number(value)] = args.as_slice() else {
                    panic!("callback should receive one numeric item");
                };
                Ok(Value::Number(*value + 1.0))
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("Array.map should succeed");

        assert_eq!(
            result,
            Value::Array(vec![Value::Number(2.0), Value::Number(3.0)])
        );
    }

    #[test]
    fn object_spread_merges_records() {
        let result = execute_native_function(
            NativeFunction::ObjectSpread,
            vec![
                Value::Record(std::collections::BTreeMap::from([(
                    "a".to_owned(),
                    Value::Number(1.0),
                )])),
                Value::Record(std::collections::BTreeMap::from([(
                    "b".to_owned(),
                    Value::Number(2.0),
                )])),
            ],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Object.spread does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("Object.spread should succeed");

        assert_eq!(
            result,
            Value::Record(BTreeMap::from([
                ("a".to_owned(), Value::Number(1.0)),
                ("b".to_owned(), Value::Number(2.0)),
            ]))
        );
    }

    #[test]
    fn json_parse_builds_runtime_values() {
        let result = execute_native_function(
            NativeFunction::JsonToObject,
            vec![Value::String(
                r#"{"name":"Ada","active":true,"scores":[1,2],"meta":null}"#.to_owned(),
            )],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Json.jsonToObject does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("Json.jsonToObject should succeed");

        assert_eq!(
            result,
            Value::Record(BTreeMap::from([
                ("active".to_owned(), Value::Boolean(true)),
                ("meta".to_owned(), Value::Null),
                ("name".to_owned(), Value::String("Ada".to_owned())),
                (
                    "scores".to_owned(),
                    Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]),
                ),
            ]))
        );
    }

    #[test]
    fn json_stringify_serializes_runtime_values() {
        let result = execute_native_function(
            NativeFunction::JsonToString,
            vec![Value::Record(BTreeMap::from([
                ("active".to_owned(), Value::Boolean(true)),
                ("name".to_owned(), Value::String("Ada".to_owned())),
            ]))],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Json.jsonToString does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("Json.jsonToString should succeed");

        assert_eq!(
            result,
            Value::String(r#"{"active":true,"name":"Ada"}"#.to_owned())
        );
    }

    #[test]
    fn json_to_object_ignores_comments_and_separator_lines() {
        let result = execute_native_function(
            NativeFunction::JsonToObject,
            vec![Value::String(
                "---\n{\n  // app name\n  \"name\": \"Ada\",\n  # enabled\n  \"active\": true,\n  /* points */\n  \"scores\": [1, 2]\n}\n".to_owned(),
            )],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Json.jsonToObject does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("relaxed JSON should parse");

        assert_eq!(
            result,
            Value::Record(BTreeMap::from([
                ("active".to_owned(), Value::Boolean(true)),
                ("name".to_owned(), Value::String("Ada".to_owned())),
                (
                    "scores".to_owned(),
                    Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]),
                ),
            ]))
        );
    }

    #[test]
    fn json_to_pretty_string_formats_nested_values() {
        let result = execute_native_function(
            NativeFunction::JsonToPrettyString,
            vec![Value::Record(BTreeMap::from([
                ("active".to_owned(), Value::Boolean(true)),
                (
                    "scores".to_owned(),
                    Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]),
                ),
            ]))],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Json.jsonToPrettyString does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("Json.jsonToPrettyString should succeed");

        assert_eq!(
            result,
            Value::String(
                "{\n  \"active\": true,\n  \"scores\": [\n    1,\n    2\n  ]\n}".to_owned()
            )
        );
    }

    #[test]
    fn logger_helpers_format_messages_and_pretty_json() {
        let logger = logger_to_value(&LoggerConfig {
            name: Some("app".to_owned()),
            level: LoggerSeverity::Debug,
            destination: LoggerDestination::Stdout,
        });

        assert_eq!(
            format_logger_line(
                &parse_logger_config(&logger, "Logger logger").expect("logger should parse"),
                LoggerSeverity::Info,
                "hello",
            ),
            "[app] INFO hello\n"
        );

        let pretty = stringify_json_value(
            &Value::Record(BTreeMap::from([(
                "name".to_owned(),
                Value::String("Ada".to_owned()),
            )])),
            JsonFormat::Pretty,
        )
        .expect("pretty JSON should serialize");
        assert_eq!(pretty, "{\n  \"name\": \"Ada\"\n}");
    }

    #[test]
    fn filesystem_functions_round_trip_files() {
        let file_path = temp_path("file.txt");
        let dir_path = file_path.parent().expect("temp file should have a parent");
        let file_path_string = file_path.to_string_lossy().into_owned();
        let dir_path_string = dir_path.to_string_lossy().into_owned();

        execute_native_function(
            NativeFunction::FilesystemWriteFile,
            vec![
                Value::String(file_path_string.clone()),
                Value::String("hello filesystem".to_owned()),
            ],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("FileSystem.writeFile does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("FileSystem.writeFile should succeed");

        let exists = execute_native_function(
            NativeFunction::FilesystemExists,
            vec![Value::String(file_path_string.clone())],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("FileSystem.exists does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("FileSystem.exists should succeed");
        assert_eq!(exists, Value::Boolean(true));

        let contents = execute_native_function(
            NativeFunction::FilesystemReadFile,
            vec![Value::String(file_path_string.clone())],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("FileSystem.readFile does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("FileSystem.readFile should succeed");
        assert_eq!(contents, Value::String("hello filesystem".to_owned()));

        let entries = execute_native_function(
            NativeFunction::FilesystemReadDir,
            vec![Value::String(dir_path_string)],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("FileSystem.readDir does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("FileSystem.readDir should succeed");

        let Value::Array(entries) = entries else {
            panic!("FileSystem.readDir should return an array");
        };
        assert!(
            entries.contains(&Value::String(
                file_path
                    .file_name()
                    .expect("temp file should have a name")
                    .to_string_lossy()
                    .into_owned()
            ))
        );

        execute_native_function(
            NativeFunction::FilesystemDeleteFile,
            vec![Value::String(file_path_string.clone())],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("FileSystem.deleteFile does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("FileSystem.deleteFile should succeed");

        let exists_after_delete = execute_native_function(
            NativeFunction::FilesystemExists,
            vec![Value::String(file_path_string)],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("FileSystem.exists does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("FileSystem.exists should succeed");
        assert_eq!(exists_after_delete, Value::Boolean(false));
    }

    #[test]
    fn filesystem_delete_reports_missing_files() {
        let path = temp_path("missing.txt");

        let error = execute_native_function(
            NativeFunction::FilesystemDeleteFile,
            vec![Value::String(path.to_string_lossy().into_owned())],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("FileSystem.deleteFile does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect_err("deleting a missing file should fail");

        assert!(error.message().contains("FileSystem.deleteFile failed for"));
    }

    #[test]
    fn filesystem_tests_cleanup_temp_files() {
        let path = temp_path("cleanup.txt");
        fs::write(&path, "temp").expect("temp file should be writable");
        fs::remove_file(&path).expect("temp file cleanup should succeed");
    }

    #[test]
    fn task_defer_wraps_zero_arg_callables_in_deferred_values() {
        let result = execute_native_function(
            NativeFunction::TaskDefer,
            vec![Value::Function(fscript_runtime::FunctionValue {
                parameters: Vec::new(),
                body: Box::new(ir::Expr::NumberLiteral {
                    value: 42.0,
                    span: Span::new(0, 0),
                }),
                environment: BTreeMap::new(),
                applied_args: Vec::new(),
                is_generator: false,
            })],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Task.defer does not execute the callable immediately")
            },
            |_value| -> Result<Value, RuntimeError> {
                unreachable!("Task.defer does not force the callable immediately")
            },
        )
        .expect("Task.defer should succeed");

        assert!(matches!(result, Value::Deferred(_)));
    }

    #[test]
    fn task_all_rejects_non_callable_values() {
        let error = execute_native_function(
            NativeFunction::TaskAll,
            vec![Value::Array(vec![Value::Number(1.0)])],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Task.all should validate before execution")
            },
            |_value| -> Result<Value, RuntimeError> {
                unreachable!("Task.all should validate before forcing")
            },
        )
        .expect_err("Task.all should reject non-callable batch items");

        assert_eq!(
            error,
            RuntimeError::new("Task.all expects zero-argument callables, found `1`")
        );
    }

    #[test]
    fn task_spawn_eagerly_starts_work_and_returns_a_deferred_handle() {
        let mut forced = 0_usize;
        let result = execute_native_function(
            NativeFunction::TaskSpawn,
            vec![Value::Function(fscript_runtime::FunctionValue {
                parameters: Vec::new(),
                body: Box::new(ir::Expr::NumberLiteral {
                    value: 42.0,
                    span: Span::new(0, 0),
                }),
                environment: BTreeMap::new(),
                applied_args: Vec::new(),
                is_generator: false,
            })],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Task.spawn starts through the force callback")
            },
            |value| -> Result<Value, RuntimeError> {
                forced += 1;
                match value {
                    Value::Deferred(deferred) => Ok(deferred
                        .outcome()
                        .map(|outcome| match outcome {
                            fscript_runtime::DeferredOutcome::Value(value) => value,
                            fscript_runtime::DeferredOutcome::Throw(value) => value,
                        })
                        .unwrap_or(Value::Number(42.0))),
                    other => Ok(other),
                }
            },
        )
        .expect("Task.spawn should succeed");

        assert_eq!(forced, 1);
        assert!(matches!(result, Value::Deferred(_)));
    }

    #[test]
    fn task_race_rejects_empty_batches() {
        let error = execute_native_function(
            NativeFunction::TaskRace,
            vec![Value::Array(Vec::new())],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Task.race should validate before execution")
            },
            |_value| -> Result<Value, RuntimeError> {
                unreachable!("Task.race should validate before forcing")
            },
        )
        .expect_err("Task.race should reject empty task batches");

        assert_eq!(
            error,
            RuntimeError::new("Task.race expects a non-empty array of tasks")
        );
    }

    #[test]
    fn object_spread_rejects_non_record_inputs() {
        let left_error = execute_native_function(
            NativeFunction::ObjectSpread,
            vec![Value::Number(1.0), Value::Record(BTreeMap::new())],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Object.spread does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect_err("left argument should be validated");
        assert_eq!(
            left_error,
            RuntimeError::new("Object.spread expects record values for its left argument")
        );

        let right_error = execute_native_function(
            NativeFunction::ObjectSpread,
            vec![Value::Record(BTreeMap::new()), Value::Number(2.0)],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Object.spread does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect_err("right argument should be validated");
        assert_eq!(
            right_error,
            RuntimeError::new("Object.spread expects record values for its right argument")
        );
    }

    #[test]
    fn array_filter_rejects_non_boolean_callbacks() {
        let error = execute_native_function(
            NativeFunction::ArrayFilter,
            vec![
                Value::String("predicate".to_owned()),
                Value::Array(vec![Value::Number(1.0)]),
            ],
            |_callee, _args| -> Result<Value, RuntimeError> {
                Ok(Value::String("nope".to_owned()))
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect_err("Array.filter callbacks must return booleans");

        assert!(
            error
                .message()
                .contains("Array.filter callbacks must return Boolean values")
        );
    }

    #[test]
    fn array_length_supports_sequences_and_rejects_scalars() {
        let sequence_length = execute_native_function(
            NativeFunction::ArrayLength,
            vec![Value::Sequence(vec![
                Value::Number(1.0),
                Value::Number(2.0),
            ])],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Array.length does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("Array.length should accept sequences");
        assert_eq!(sequence_length, Value::Number(2.0));

        let error = execute_native_function(
            NativeFunction::ArrayLength,
            vec![Value::Boolean(true)],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Array.length does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect_err("Array.length should reject scalars");
        assert!(
            error
                .message()
                .contains("Array.length expects an array value")
        );
    }

    #[test]
    fn json_parse_reports_trailing_content() {
        let error = execute_native_function(
            NativeFunction::JsonToObject,
            vec![Value::String("true false".to_owned())],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Json.jsonToObject does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect_err("trailing content should fail");

        assert!(
            error
                .message()
                .contains("trailing content after the first JSON value")
        );
    }

    #[test]
    fn string_and_number_helpers_validate_arguments() {
        let trim_error = execute_native_function(
            NativeFunction::StringTrim,
            vec![Value::Number(1.0)],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("string helpers do not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect_err("String.trim should reject non-strings");
        assert_eq!(
            trim_error,
            RuntimeError::new("string helpers expect a String argument")
        );

        let digits = execute_native_function(
            NativeFunction::StringIsDigits,
            vec![Value::String("12345".to_owned())],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("String.isDigits does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("String.isDigits should succeed");
        assert_eq!(digits, Value::Boolean(true));

        let parse_error = execute_native_function(
            NativeFunction::NumberParse,
            vec![Value::String("abc".to_owned())],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Number.parse does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect_err("Number.parse should reject invalid numbers");
        assert!(
            parse_error
                .message()
                .contains("could not parse `abc` as a Number")
        );
    }

    #[test]
    fn result_helpers_cover_tag_checks_and_defaults() {
        let ok_result = execute_native_function(
            NativeFunction::ResultOk,
            vec![Value::String("value".to_owned())],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Result.ok does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("Result.ok should succeed");

        let is_ok = execute_native_function(
            NativeFunction::ResultIsOk,
            vec![ok_result.clone()],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Result.isOk does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("Result.isOk should succeed");
        assert_eq!(is_ok, Value::Boolean(true));

        let value = execute_native_function(
            NativeFunction::ResultWithDefault,
            vec![Value::String("fallback".to_owned()), ok_result],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Result.withDefault does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect("Result.withDefault should unwrap ok values");
        assert_eq!(value, Value::String("value".to_owned()));

        let invalid_ok = Value::Record(BTreeMap::from([(
            "tag".to_owned(),
            Value::String("ok".to_owned()),
        )]));
        let missing_value_error = execute_native_function(
            NativeFunction::ResultWithDefault,
            vec![Value::String("fallback".to_owned()), invalid_ok],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Result.withDefault does not call back")
            },
            |value| -> Result<Value, RuntimeError> { Ok(value) },
        )
        .expect_err("ok results need a value field");
        assert_eq!(
            missing_value_error,
            RuntimeError::new("ok results must contain a `value` field")
        );
    }

    #[test]
    fn task_helpers_cover_success_and_force_paths() {
        let zero_arg_native = Value::NativeFunction(NativeFunctionValue {
            function: NativeFunction::ArrayLength,
            applied_args: vec![Value::Array(vec![Value::Number(1.0)])],
        });

        let batch = execute_native_function(
            NativeFunction::TaskAll,
            vec![Value::Array(vec![zero_arg_native.clone()])],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Task.all should not execute immediately")
            },
            |_value| -> Result<Value, RuntimeError> {
                unreachable!("Task.all should not force immediately")
            },
        )
        .expect("Task.all should create a deferred batch");
        assert_eq!(
            batch,
            Value::Deferred(DeferredValue::new(DeferredBody::Batch(vec![
                zero_arg_native.clone()
            ])))
        );

        let race = execute_native_function(
            NativeFunction::TaskRace,
            vec![Value::Array(vec![zero_arg_native.clone()])],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Task.race should not execute immediately")
            },
            |_value| -> Result<Value, RuntimeError> {
                unreachable!("Task.race should not force immediately")
            },
        )
        .expect("Task.race should create a deferred race");
        assert_eq!(
            race,
            Value::Deferred(DeferredValue::new(DeferredBody::Race(vec![
                zero_arg_native.clone()
            ])))
        );

        let mut forced = Vec::new();
        let forced_value = execute_native_function(
            NativeFunction::TaskForce,
            vec![Value::Deferred(DeferredValue::new(DeferredBody::Call(
                Box::new(Value::Number(42.0)),
            )))],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Task.force does not call through the callable callback")
            },
            |value| -> Result<Value, RuntimeError> {
                forced.push(value.clone());
                Ok(Value::String("forced".to_owned()))
            },
        )
        .expect("Task.force should delegate to the force callback");
        assert_eq!(forced.len(), 1);
        assert_eq!(forced_value, Value::String("forced".to_owned()));
    }

    #[test]
    fn task_defer_rejects_callables_that_still_need_arguments() {
        let error = execute_native_function(
            NativeFunction::TaskDefer,
            vec![Value::NativeFunction(NativeFunctionValue::new(
                NativeFunction::ArrayLength,
            ))],
            |_callee, _args| -> Result<Value, RuntimeError> {
                unreachable!("Task.defer validates before execution")
            },
            |_value| -> Result<Value, RuntimeError> {
                unreachable!("Task.defer validates before forcing")
            },
        )
        .expect_err("Task.defer should reject non-zero-arg callables");

        assert!(
            error
                .message()
                .contains("Task.defer expects zero-argument callables")
        );
    }

    #[test]
    fn http_option_and_response_helpers_validate_records() {
        let bad_port = parse_http_options(Value::Record(BTreeMap::from([
            ("host".to_owned(), Value::String("127.0.0.1".to_owned())),
            ("port".to_owned(), Value::Number(70000.0)),
            ("maxRequests".to_owned(), Value::Number(1.0)),
        ])))
        .expect_err("ports must stay in range");
        assert!(
            bad_port
                .message()
                .contains("options.port must be an integer between 0 and 65535")
        );

        let bad_status = parse_http_response(Value::Record(BTreeMap::from([
            ("status".to_owned(), Value::Number(99.0)),
            (
                "contentType".to_owned(),
                Value::String("text/plain".to_owned()),
            ),
            ("body".to_owned(), Value::String("hello".to_owned())),
        ])))
        .expect_err("status codes must stay in range");
        assert!(
            bad_status
                .message()
                .contains("response.status must be an integer between 100 and 599")
        );
    }

    #[test]
    fn read_http_request_reports_incomplete_requests() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = listener
            .local_addr()
            .expect("listener should expose a local address");
        let sender = std::thread::spawn(move || {
            let mut stream =
                std::net::TcpStream::connect(address).expect("client should connect to listener");
            use std::io::Write;
            stream
                .write_all(b"GET / HTTP/1.1\r\nHost: example.test")
                .expect("client request should write");
        });

        let (mut stream, _) = listener
            .accept()
            .expect("listener should accept a connection");
        let error = read_http_request(&mut stream).expect_err("missing terminator should fail");
        sender.join().expect("client thread should finish");

        assert!(
            error
                .message()
                .contains("Http.serve received an incomplete HTTP request")
        );
    }
}
