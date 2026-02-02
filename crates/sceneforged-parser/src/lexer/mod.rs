//! Logos-based lexer for release names.
//!
//! This module provides tokenization using the [logos](https://docs.rs/logos) crate,
//! which generates a fast lexer from regex patterns at compile time.

mod token;
pub use token::Token;

use logos::Logos;
use std::ops::Range;

/// Byte span in the input string.
///
/// Represents a range of bytes in the original input, used for tracking
/// token positions and extracting substrings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
}

#[allow(dead_code)]
impl Span {
    /// Create a new span.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Length of the span in bytes.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Check if span is empty.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Check if this span overlaps with another.
    pub fn overlaps(&self, other: &Span) -> bool {
        self.start < other.end && other.start < self.end
    }
}

impl From<Range<usize>> for Span {
    fn from(range: Range<usize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

/// A detected bracket group in the input.
///
/// Represents a matched pair of brackets (either `[...]` or `(...)`) with
/// spans for both the entire group and the content inside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketGroup {
    /// The span of the entire bracket group including brackets.
    pub outer_span: Span,
    /// The span of the content inside the brackets.
    pub inner_span: Span,
    /// The bracket character used ('[' or '(').
    pub bracket_char: char,
}

/// Find all bracket groups in the input.
///
/// Returns groups with their spans for both `[...]` and `(...)`.
/// Handles nested and multiple bracket groups correctly.
pub fn find_bracket_groups(input: &str) -> Vec<BracketGroup> {
    let mut groups = Vec::new();
    let mut stack: Vec<(usize, char)> = Vec::new();

    for (i, ch) in input.char_indices() {
        match ch {
            '[' | '(' => {
                stack.push((i, ch));
            }
            ']' => {
                if let Some((start, '[')) = stack.pop() {
                    groups.push(BracketGroup {
                        outer_span: Span::new(start, i + 1),
                        inner_span: Span::new(start + 1, i),
                        bracket_char: '[',
                    });
                }
            }
            ')' => {
                if let Some((start, '(')) = stack.pop() {
                    groups.push(BracketGroup {
                        outer_span: Span::new(start, i + 1),
                        inner_span: Span::new(start + 1, i),
                        bracket_char: '(',
                    });
                }
            }
            _ => {}
        }
    }

    groups
}

/// Kind of token for compatibility with the old tokenizer API.
///
/// This enum provides a simpler classification of tokens for use by
/// extractors that don't need to distinguish between specific patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Regular word token.
    Word,
    /// Delimiter (., _, -, space).
    Delimiter,
    /// Opening bracket ([, ().
    BracketOpen,
    /// Closing bracket (], )).
    BracketClose,
    /// Numeric token.
    Number,
}

/// A compatibility wrapper around Logos tokens.
///
/// Provides the same API as the old hand-rolled tokenizer for backward compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyToken<'input> {
    /// The text of this token (borrowed from input).
    pub text: &'input str,
    /// Byte span in the original input.
    pub span: Span,
    /// Kind of token.
    pub kind: TokenKind,
    /// Whether this token is inside brackets.
    pub in_brackets: bool,
}

impl<'input> LegacyToken<'input> {
    /// Create a new token.
    pub fn new(text: &'input str, span: Span, kind: TokenKind, in_brackets: bool) -> Self {
        Self {
            text,
            span,
            kind,
            in_brackets,
        }
    }
}

/// A lexer that tokenizes release names using Logos.
///
/// This struct wraps the Logos-generated lexer and provides additional
/// functionality like bracket tracking and compatibility with the old API.
#[allow(dead_code)]
pub struct Lexer<'src> {
    tokens: Vec<(Token<'src>, Range<usize>)>,
    input: &'src str,
}

#[allow(dead_code)]
impl<'src> Lexer<'src> {
    /// Create a new lexer for the given input.
    ///
    /// Tokenizes the entire input string immediately using Logos.
    pub fn new(input: &'src str) -> Self {
        let tokens: Vec<_> = Token::lexer(input)
            .spanned()
            .filter_map(|(tok, span)| tok.ok().map(|t| (t, span)))
            .collect();
        Self { tokens, input }
    }

    /// Get all tokens with their spans.
    pub fn tokens(&self) -> &[(Token<'src>, Range<usize>)] {
        &self.tokens
    }

    /// Get the original input string.
    pub fn input(&self) -> &'src str {
        self.input
    }
}

/// A token stream providing compatibility with the old tokenizer API.
///
/// This struct converts Logos tokens into the format expected by existing
/// extractors and pipeline code.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TokenStream<'input> {
    tokens: Vec<LegacyToken<'input>>,
    input: &'input str,
}

#[allow(dead_code)]
impl<'input> TokenStream<'input> {
    /// Tokenize an input string.
    ///
    /// Creates a token stream that maintains compatibility with the old API,
    /// including bracket tracking and delimiter handling.
    pub fn new(input: &'input str) -> Self {
        let bracket_groups = find_bracket_groups(input);
        let lexer = Lexer::new(input);

        let mut tokens = Vec::new();

        for (token, span) in lexer.tokens() {
            let in_brackets = bracket_groups
                .iter()
                .any(|g| span.start > g.outer_span.start && span.end <= g.outer_span.end);

            let span_obj = Span::from(span.clone());
            let text = &input[span.clone()];

            // Map Logos tokens to legacy token kinds
            let kind = match token {
                Token::Dot | Token::Hyphen | Token::Underscore => TokenKind::Delimiter,
                Token::BracketOpen | Token::ParenOpen => TokenKind::BracketOpen,
                Token::BracketClose | Token::ParenClose => TokenKind::BracketClose,
                Token::Number(_) => TokenKind::Number,
                _ => TokenKind::Word,
            };

            tokens.push(LegacyToken::new(text, span_obj, kind, in_brackets));
        }

        Self { tokens, input }
    }

    /// Get the original input string.
    pub fn input(&self) -> &'input str {
        self.input
    }

    /// Get all tokens.
    pub fn tokens(&self) -> &[LegacyToken<'input>] {
        &self.tokens
    }

    /// Get tokens that are not delimiters.
    pub fn content_tokens(&self) -> impl Iterator<Item = &LegacyToken<'input>> {
        self.tokens
            .iter()
            .filter(|t| !matches!(t.kind, TokenKind::Delimiter))
    }

    /// Check if a byte position is inside any bracket group.
    pub fn is_in_brackets(&self, pos: usize) -> bool {
        self.tokens
            .iter()
            .any(|t| t.span.start <= pos && pos < t.span.end && t.in_brackets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_basic() {
        let lexer = Lexer::new("The.Matrix.1999.1080p.BluRay.x264");
        assert!(!lexer.tokens().is_empty());
    }

    #[test]
    fn test_simple_dotted_name() {
        let stream = TokenStream::new("The.Matrix.1999.BluRay");
        let words: Vec<_> = stream.content_tokens().map(|t| t.text).collect();
        assert_eq!(words, vec!["The", "Matrix", "1999", "BluRay"]);
    }

    #[test]
    fn test_bracket_groups() {
        let stream = TokenStream::new("[SubGroup] Anime Name - 01 [1080p] [CRC32].mkv");
        let words: Vec<_> = stream.content_tokens().map(|t| t.text).collect();
        assert!(words.contains(&"SubGroup"));
        assert!(words.contains(&"Anime"));
        assert!(words.contains(&"1080p"));
        assert!(words.contains(&"CRC32"));
    }

    #[test]
    fn test_in_brackets_flag() {
        let stream = TokenStream::new("[Group] Title");
        let group_token = stream.tokens().iter().find(|t| t.text == "Group").unwrap();
        assert!(group_token.in_brackets);
        let title_token = stream.tokens().iter().find(|t| t.text == "Title").unwrap();
        assert!(!title_token.in_brackets);
    }

    #[test]
    fn test_mixed_delimiters() {
        let stream = TokenStream::new("Movie_(2020)_720p");
        let words: Vec<_> = stream.content_tokens().map(|t| t.text).collect();
        assert!(words.contains(&"Movie"));
        assert!(words.contains(&"2020"));
        assert!(words.contains(&"720p"));
    }

    #[test]
    fn test_number_vs_word() {
        let stream = TokenStream::new("Show.S01E05.x264");
        let tokens: Vec<_> = stream
            .tokens()
            .iter()
            .filter(|t| t.kind == TokenKind::Number || t.kind == TokenKind::Word)
            .collect();
        // S01E05 should be recognized as SeasonEpisode (Word)
        // x264 should be recognized as CodecH264 (Word)
        assert!(tokens.iter().any(|t| t.text.contains("S01E05")));
        assert!(tokens.iter().any(|t| t.text.contains("x264")));
    }

    #[test]
    fn test_bracket_group_detection() {
        let groups = find_bracket_groups("[a][b](c)");
        assert_eq!(groups.len(), 3);
    }

    #[test]
    fn test_span_operations() {
        let span1 = Span::new(0, 5);
        let span2 = Span::new(3, 8);
        let span3 = Span::new(10, 15);

        assert_eq!(span1.len(), 5);
        assert!(!span1.is_empty());
        assert!(span1.overlaps(&span2));
        assert!(!span1.overlaps(&span3));
    }

    #[test]
    fn test_season_episode_single_digit() {
        // Test both single and double digit seasons
        let inputs = ["S6E02", "S06E02", "S1E1", "S01E05", "S6E02-Unwrapped"];
        for input in inputs {
            let lexer = Lexer::new(input);
            let has_season_episode = lexer
                .tokens()
                .iter()
                .any(|(t, _)| matches!(t, Token::SeasonEpisode(_)));
            assert!(
                has_season_episode,
                "{} should contain SeasonEpisode token",
                input
            );
        }
    }
}
