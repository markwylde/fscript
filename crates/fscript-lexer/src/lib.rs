//! Tokenization for FScript source files.

use fscript_source::{SourceFile, Span};

/// A token produced by the lexer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    /// Token kind.
    pub kind: TokenKind,
    /// Token span in the original source text.
    pub span: Span,
}

/// All tokens needed by the current frontend slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Whitespace,
    LineComment,
    BlockComment,
    Identifier,
    NumberLiteral,
    StringLiteral,
    Import,
    From,
    Export,
    Type,
    If,
    Else,
    Match,
    Try,
    Catch,
    Throw,
    Defer,
    Yield,
    True,
    False,
    NumberType,
    StringType,
    BooleanType,
    Null,
    Undefined,
    NeverType,
    UnknownType,
    Assign,
    Arrow,
    Pipe,
    LogicalOr,
    LogicalAnd,
    NullishCoalesce,
    StrictEqual,
    StrictNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    Union,
    Intersection,
    Dot,
    Colon,
    Comma,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
}

/// A lexical diagnostic with source span information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexDiagnostic {
    /// Error kind.
    pub kind: LexDiagnosticKind,
    /// Error span.
    pub span: Span,
}

/// A specific lexical failure kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LexDiagnosticKind {
    InvalidToken(char),
    UnterminatedString,
    InvalidEscape(char),
    UnterminatedBlockComment,
}

/// Result of tokenizing a source file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LexedFile {
    /// Tokens in source order.
    pub tokens: Vec<Token>,
    /// Diagnostics encountered during lexing.
    pub diagnostics: Vec<LexDiagnostic>,
}

/// Lexes an FScript source file.
#[must_use]
pub fn lex(source: &SourceFile) -> LexedFile {
    let text = source.contents();
    let mut lexer = Lexer::new(text);
    let mut result = LexedFile::default();

    while let Some(item) = lexer.next() {
        match item {
            Ok(token) => result.tokens.push(token),
            Err(diagnostic) => result.diagnostics.push(diagnostic),
        }
    }

    result
}

struct Lexer<'a> {
    source: &'a str,
    offset: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, offset: 0 }
    }

    fn next(&mut self) -> Option<Result<Token, LexDiagnostic>> {
        if self.offset >= self.source.len() {
            return None;
        }

        let start = self.offset;
        let remaining = &self.source[self.offset..];
        let first = remaining.chars().next()?;

        if first.is_ascii_whitespace() {
            self.offset += scan_while(remaining, |ch| ch.is_ascii_whitespace());
            return Some(Ok(Token {
                kind: TokenKind::Whitespace,
                span: Span::new(start, self.offset),
            }));
        }

        if let Some(rest) = remaining.strip_prefix("//") {
            let consumed = rest.find('\n').unwrap_or(rest.len());
            self.offset += 2 + consumed;
            return Some(Ok(Token {
                kind: TokenKind::LineComment,
                span: Span::new(start, self.offset),
            }));
        }

        if remaining.starts_with("/*") {
            if let Some(end) = remaining.find("*/") {
                self.offset += end + 2;
                return Some(Ok(Token {
                    kind: TokenKind::BlockComment,
                    span: Span::new(start, self.offset),
                }));
            }

            self.offset = self.source.len();
            return Some(Err(LexDiagnostic {
                kind: LexDiagnosticKind::UnterminatedBlockComment,
                span: Span::new(start, self.offset),
            }));
        }

        if matches!(first, '"' | '\'') {
            return Some(self.lex_string(first, start));
        }

        if is_identifier_start(first) {
            self.offset += first.len_utf8();
            self.offset += scan_while(&self.source[self.offset..], is_identifier_continue);
            let span = Span::new(start, self.offset);
            let kind = keyword_kind(span.slice(self.source)).unwrap_or(TokenKind::Identifier);

            return Some(Ok(Token { kind, span }));
        }

        if first.is_ascii_digit() {
            self.offset += first.len_utf8();
            self.offset += scan_number(&self.source[self.offset..]);

            return Some(Ok(Token {
                kind: TokenKind::NumberLiteral,
                span: Span::new(start, self.offset),
            }));
        }

        if let Some((kind, width)) = punctuation_kind(remaining) {
            self.offset += width;
            return Some(Ok(Token {
                kind,
                span: Span::new(start, self.offset),
            }));
        }

        self.offset += first.len_utf8();
        Some(Err(LexDiagnostic {
            kind: LexDiagnosticKind::InvalidToken(first),
            span: Span::new(start, self.offset),
        }))
    }

    fn lex_string(&mut self, quote: char, start: usize) -> Result<Token, LexDiagnostic> {
        self.offset += quote.len_utf8();

        while self.offset < self.source.len() {
            let current = self.source[self.offset..]
                .chars()
                .next()
                .expect("offset is kept on a char boundary");

            if current == quote {
                self.offset += current.len_utf8();
                return Ok(Token {
                    kind: TokenKind::StringLiteral,
                    span: Span::new(start, self.offset),
                });
            }

            if current == '\n' {
                return Err(LexDiagnostic {
                    kind: LexDiagnosticKind::UnterminatedString,
                    span: Span::new(start, self.offset),
                });
            }

            if current == '\\' {
                let escape_offset = self.offset;
                self.offset += 1;

                if self.offset >= self.source.len() {
                    return Err(LexDiagnostic {
                        kind: LexDiagnosticKind::UnterminatedString,
                        span: Span::new(start, self.offset),
                    });
                }

                let escaped = self.source[self.offset..]
                    .chars()
                    .next()
                    .expect("offset is kept on a char boundary");

                if !matches!(escaped, '\\' | '\'' | '"' | 'n' | 'r' | 't' | '0') {
                    self.offset += escaped.len_utf8();
                    self.consume_string_tail(quote);
                    return Err(LexDiagnostic {
                        kind: LexDiagnosticKind::InvalidEscape(escaped),
                        span: Span::new(escape_offset, self.offset),
                    });
                }

                self.offset += escaped.len_utf8();
                continue;
            }

            self.offset += current.len_utf8();
        }

        Err(LexDiagnostic {
            kind: LexDiagnosticKind::UnterminatedString,
            span: Span::new(start, self.offset),
        })
    }

    fn consume_string_tail(&mut self, quote: char) {
        while self.offset < self.source.len() {
            let current = self.source[self.offset..]
                .chars()
                .next()
                .expect("offset is kept on a char boundary");

            self.offset += current.len_utf8();

            if current == quote || current == '\n' {
                break;
            }
        }
    }
}

fn scan_while(source: &str, predicate: impl Fn(char) -> bool) -> usize {
    let mut consumed = 0;

    for ch in source.chars() {
        if !predicate(ch) {
            break;
        }

        consumed += ch.len_utf8();
    }

    consumed
}

fn scan_number(source: &str) -> usize {
    let mut consumed = scan_while(source, |ch| ch.is_ascii_digit());
    let remainder = &source[consumed..];

    if let Some(rest) = remainder.strip_prefix('.') {
        let fractional = scan_while(rest, |ch| ch.is_ascii_digit());
        if fractional > 0 {
            consumed += 1 + fractional;
        }
    }

    consumed
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn keyword_kind(value: &str) -> Option<TokenKind> {
    Some(match value {
        "import" => TokenKind::Import,
        "from" => TokenKind::From,
        "export" => TokenKind::Export,
        "type" => TokenKind::Type,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "match" => TokenKind::Match,
        "try" => TokenKind::Try,
        "catch" => TokenKind::Catch,
        "throw" => TokenKind::Throw,
        "defer" => TokenKind::Defer,
        "yield" => TokenKind::Yield,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "Number" => TokenKind::NumberType,
        "String" => TokenKind::StringType,
        "Boolean" => TokenKind::BooleanType,
        "Null" => TokenKind::Null,
        "Undefined" => TokenKind::Undefined,
        "Never" => TokenKind::NeverType,
        "Unknown" => TokenKind::UnknownType,
        _ => return None,
    })
}

fn punctuation_kind(source: &str) -> Option<(TokenKind, usize)> {
    Some(if source.starts_with("===") {
        (TokenKind::StrictEqual, 3)
    } else if source.starts_with("!==") {
        (TokenKind::StrictNotEqual, 3)
    } else if source.starts_with("=>") {
        (TokenKind::Arrow, 2)
    } else if source.starts_with("|>") {
        (TokenKind::Pipe, 2)
    } else if source.starts_with("||") {
        (TokenKind::LogicalOr, 2)
    } else if source.starts_with("&&") {
        (TokenKind::LogicalAnd, 2)
    } else if source.starts_with("??") {
        (TokenKind::NullishCoalesce, 2)
    } else if source.starts_with("<=") {
        (TokenKind::LessEqual, 2)
    } else if source.starts_with(">=") {
        (TokenKind::GreaterEqual, 2)
    } else {
        match source.chars().next()? {
            '=' => (TokenKind::Assign, 1),
            '<' => (TokenKind::Less, 1),
            '>' => (TokenKind::Greater, 1),
            '+' => (TokenKind::Plus, 1),
            '-' => (TokenKind::Minus, 1),
            '*' => (TokenKind::Star, 1),
            '/' => (TokenKind::Slash, 1),
            '%' => (TokenKind::Percent, 1),
            '!' => (TokenKind::Bang, 1),
            '|' => (TokenKind::Union, 1),
            '&' => (TokenKind::Intersection, 1),
            '.' => (TokenKind::Dot, 1),
            ':' => (TokenKind::Colon, 1),
            ',' => (TokenKind::Comma, 1),
            '(' => (TokenKind::LeftParen, 1),
            ')' => (TokenKind::RightParen, 1),
            '{' => (TokenKind::LeftBrace, 1),
            '}' => (TokenKind::RightBrace, 1),
            '[' => (TokenKind::LeftBracket, 1),
            ']' => (TokenKind::RightBracket, 1),
            _ => return None,
        }
    })
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use fscript_source::{SourceFile, Span};
    use insta::assert_snapshot;
    use proptest::prelude::*;

    use super::{LexDiagnostic, LexDiagnosticKind, Token, TokenKind, keyword_kind, lex};

    fn source(text: &str) -> SourceFile {
        SourceFile::new(Utf8PathBuf::from("test.fs"), text.to_owned())
    }

    #[test]
    fn lexes_keywords_symbols_literals_and_trivia() {
        let result = lex(&source("import value = [1, 'ok'] // comment"));

        let kinds: Vec<_> = result.tokens.iter().map(|token| token.kind).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Import,
                TokenKind::Whitespace,
                TokenKind::Identifier,
                TokenKind::Whitespace,
                TokenKind::Assign,
                TokenKind::Whitespace,
                TokenKind::LeftBracket,
                TokenKind::NumberLiteral,
                TokenKind::Comma,
                TokenKind::Whitespace,
                TokenKind::StringLiteral,
                TokenKind::RightBracket,
                TokenKind::Whitespace,
                TokenKind::LineComment,
            ]
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn lexes_multichar_operators_with_longest_match_rules() {
        let result = lex(&source("a === b !== c <= d >= e |> f ?? g && h || i => j"));

        let kinds: Vec<_> = result
            .tokens
            .into_iter()
            .filter(|token| token.kind != TokenKind::Whitespace)
            .map(|token| token.kind)
            .collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Identifier,
                TokenKind::StrictEqual,
                TokenKind::Identifier,
                TokenKind::StrictNotEqual,
                TokenKind::Identifier,
                TokenKind::LessEqual,
                TokenKind::Identifier,
                TokenKind::GreaterEqual,
                TokenKind::Identifier,
                TokenKind::Pipe,
                TokenKind::Identifier,
                TokenKind::NullishCoalesce,
                TokenKind::Identifier,
                TokenKind::LogicalAnd,
                TokenKind::Identifier,
                TokenKind::LogicalOr,
                TokenKind::Identifier,
                TokenKind::Arrow,
                TokenKind::Identifier,
            ]
        );
    }

    #[test]
    fn reports_invalid_escape_sequences() {
        let result = lex(&source("'\\x'"));

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].kind,
            LexDiagnosticKind::InvalidEscape('x')
        );
    }

    #[test]
    fn reports_unterminated_block_comments() {
        let result = lex(&source("/* never closes"));

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].kind,
            LexDiagnosticKind::UnterminatedBlockComment
        );
    }

    #[test]
    fn lexes_closed_block_comments_and_decimal_numbers() {
        let result = lex(&source("/* ok */ 3.14"));

        assert_eq!(
            result.tokens,
            vec![
                Token {
                    kind: TokenKind::BlockComment,
                    span: Span::new(0, 8),
                },
                Token {
                    kind: TokenKind::Whitespace,
                    span: Span::new(8, 9),
                },
                Token {
                    kind: TokenKind::NumberLiteral,
                    span: Span::new(9, 13),
                },
            ]
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn reports_unterminated_strings_at_newlines_and_after_trailing_escapes() {
        let newline = lex(&source("'line\nnext"));
        let trailing_escape = lex(&source("'\\"));

        assert_eq!(newline.diagnostics.len(), 1);
        assert_eq!(
            newline.diagnostics[0].kind,
            LexDiagnosticKind::UnterminatedString
        );
        assert_eq!(trailing_escape.diagnostics.len(), 1);
        assert_eq!(
            trailing_escape.diagnostics[0].kind,
            LexDiagnosticKind::UnterminatedString
        );
    }

    #[test]
    fn accepts_strings_with_supported_escape_sequences() {
        let result = lex(&source("'line\\n'"));

        assert_eq!(
            result.tokens,
            vec![Token {
                kind: TokenKind::StringLiteral,
                span: Span::new(0, 8),
            }]
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn snapshots_tokens_and_diagnostics_for_representative_source() {
        let result = lex(&source(
            "import Array from 'std:array'\n\
             values = [1, 2, '3'] |> Array.map((value: Number): Number => value + 1)\n\
             broken = '\\x'\n",
        ));

        assert_snapshot!(
            "representative_lexed_file",
            format_lexed(&result.tokens, &result.diagnostics)
        );
    }

    proptest! {
        #[test]
        fn lexed_items_have_contiguous_non_overlapping_spans(text in any::<String>()) {
            let result = lex(&source(&text));
            let mut spans: Vec<_> = result
                .tokens
                .iter()
                .map(|token| (token.span.start(), token.span.end()))
                .chain(
                    result
                        .diagnostics
                        .iter()
                        .map(|diagnostic| (diagnostic.span.start(), diagnostic.span.end())),
                )
                .collect();
            spans.sort_unstable();

            let mut cursor = 0;
            for (start, end) in spans {
                prop_assert!(start >= cursor);
                prop_assert!(start < end);
                prop_assert!(end <= text.len());
                prop_assert!(text.is_char_boundary(start));
                prop_assert!(text.is_char_boundary(end));
                cursor = end;
            }
        }

        #[test]
        fn identifier_like_inputs_lex_as_a_single_identifierish_token(identifier in identifier_strategy()) {
            let result = lex(&source(&identifier));

            prop_assert!(result.diagnostics.is_empty());
            prop_assert_eq!(result.tokens.len(), 1);
            prop_assert_eq!(result.tokens[0].span, Span::new(0, identifier.len()));

            let expected_kind = keyword_kind(&identifier).unwrap_or(TokenKind::Identifier);
            prop_assert_eq!(result.tokens[0].kind, expected_kind);
        }
    }

    fn identifier_strategy() -> impl Strategy<Value = String> {
        (
            prop_oneof![
                Just('_'),
                proptest::char::range('a', 'z'),
                proptest::char::range('A', 'Z')
            ],
            proptest::collection::vec(
                prop_oneof![
                    Just('_'),
                    proptest::char::range('a', 'z'),
                    proptest::char::range('A', 'Z'),
                    proptest::char::range('0', '9')
                ],
                0..31,
            ),
        )
            .prop_map(|(head, tail)| std::iter::once(head).chain(tail).collect())
    }

    fn format_lexed(tokens: &[Token], diagnostics: &[LexDiagnostic]) -> String {
        let mut output = String::new();
        output.push_str("tokens:\n");
        for token in tokens {
            output.push_str(&format!(
                "- {:?} {:?}\n",
                token.kind,
                (token.span.start(), token.span.end())
            ));
        }

        output.push_str("diagnostics:\n");
        for diagnostic in diagnostics {
            output.push_str(&format!(
                "- {:?} {:?}\n",
                diagnostic.kind,
                (diagnostic.span.start(), diagnostic.span.end())
            ));
        }

        output
    }
}
