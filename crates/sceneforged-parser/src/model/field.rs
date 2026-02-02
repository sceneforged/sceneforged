//! Field-level parsing types with confidence tracking.
//!
//! This module provides the core `ParsedField<T>` wrapper that tracks not just
//! the parsed value, but also metadata about how it was parsed: confidence level,
//! source span, raw text, and alternative interpretations.

/// Float-based confidence score (0.0-1.0).
///
/// Represents how confident the parser is about a particular field value.
/// Higher values indicate greater confidence, with 1.0 being absolutely certain.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Confidence(f32);

impl Default for Confidence {
    fn default() -> Self {
        Self::MEDIUM
    }
}

impl Confidence {
    /// Absolutely certain - unambiguous pattern match.
    pub const CERTAIN: Confidence = Confidence(1.0);

    /// High confidence - very likely correct based on strong patterns.
    pub const HIGH: Confidence = Confidence(0.9);

    /// Medium confidence - likely correct but some ambiguity.
    pub const MEDIUM: Confidence = Confidence(0.7);

    /// Low confidence - uncertain, multiple interpretations possible.
    pub const LOW: Confidence = Confidence(0.5);

    /// Wild guess - needs external validation.
    pub const GUESS: Confidence = Confidence(0.3);

    /// Creates a new confidence value, clamping to the valid range [0.0, 1.0].
    ///
    /// # Example
    /// ```
    /// # use sceneforged_parser::model::Confidence;
    /// let conf = Confidence::new(0.85);
    /// assert_eq!(conf.value(), 0.85);
    ///
    /// // Values are clamped
    /// let clamped = Confidence::new(1.5);
    /// assert_eq!(clamped.value(), 1.0);
    /// ```
    pub fn new(value: f32) -> Self {
        Confidence(value.clamp(0.0, 1.0))
    }

    /// Returns the raw confidence value as a float in [0.0, 1.0].
    pub fn value(&self) -> f32 {
        self.0
    }

    /// Returns true if this confidence level suggests external validation is needed.
    ///
    /// Confidence below 0.5 indicates the parser is more uncertain than certain
    /// and the result should be reviewed or validated against external sources.
    pub fn needs_review(&self) -> bool {
        self.0 < 0.5
    }

    /// Returns true if this confidence level is reasonably high (>= 0.7).
    ///
    /// Values at or above MEDIUM confidence are generally reliable enough
    /// to use without additional validation.
    pub fn is_confident(&self) -> bool {
        self.0 >= 0.7
    }
}

/// A parsed field with comprehensive metadata.
///
/// Wraps a parsed value of type `T` along with information about how it was
/// extracted: confidence score, source location, raw text, and alternative
/// interpretations that were considered.
///
/// # Type Parameters
/// - `T`: The type of the parsed value (e.g., `String`, `u32`, `Resolution`)
///
/// # Example
/// ```
/// # use sceneforged_parser::model::{ParsedField, Confidence};
/// let year = ParsedField::new(2024, Confidence::HIGH, (10, 14), "2024");
/// assert_eq!(*year, 2024);
/// assert_eq!(year.confidence, Confidence::HIGH);
/// assert_eq!(year.raw, "2024");
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParsedField<T> {
    /// The parsed value.
    pub value: T,

    /// Confidence score for this interpretation.
    pub confidence: Confidence,

    /// Byte span in the original input string (start, end).
    pub span: (usize, usize),

    /// Raw text that was parsed to produce this value.
    pub raw: String,

    /// Alternative interpretations that were considered but rejected.
    /// These may be useful for fallback or user disambiguation.
    pub alternatives: Vec<T>,
}

impl<T> ParsedField<T> {
    /// Creates a new parsed field with the given value and metadata.
    ///
    /// # Example
    /// ```
    /// # use sceneforged_parser::model::{ParsedField, Confidence};
    /// let field = ParsedField::new(
    ///     1080,
    ///     Confidence::CERTAIN,
    ///     (5, 9),
    ///     "1080p"
    /// );
    /// ```
    pub fn new(
        value: T,
        confidence: Confidence,
        span: (usize, usize),
        raw: impl Into<String>,
    ) -> Self {
        Self {
            value,
            confidence,
            span,
            raw: raw.into(),
            alternatives: Vec::new(),
        }
    }

    /// Creates a new parsed field with maximum confidence (CERTAIN).
    ///
    /// Convenience method for cases where the parse is unambiguous.
    ///
    /// # Example
    /// ```
    /// # use sceneforged_parser::model::{ParsedField, Confidence};
    /// let field = ParsedField::certain(2024, (0, 4), "2024");
    /// assert_eq!(field.confidence, Confidence::CERTAIN);
    /// ```
    pub fn certain(value: T, span: (usize, usize), raw: impl Into<String>) -> Self {
        Self::new(value, Confidence::CERTAIN, span, raw)
    }

    /// Adds alternative interpretations to this field.
    ///
    /// Returns self for method chaining.
    ///
    /// # Example
    /// ```
    /// # use sceneforged_parser::model::{ParsedField, Confidence};
    /// let field = ParsedField::new(1920, Confidence::HIGH, (0, 4), "1920")
    ///     .with_alternatives(vec![1080, 720]);
    /// assert_eq!(field.alternatives.len(), 2);
    /// ```
    pub fn with_alternatives(mut self, alts: Vec<T>) -> Self {
        self.alternatives = alts;
        self
    }

    /// Transforms the value using the given function, preserving metadata.
    ///
    /// This allows converting between types while maintaining confidence,
    /// span, and raw text information.
    ///
    /// # Example
    /// ```
    /// # use sceneforged_parser::model::{ParsedField, Confidence};
    /// let num_field = ParsedField::new(42, Confidence::HIGH, (0, 2), "42");
    /// let str_field = num_field.map(|n| n.to_string());
    /// assert_eq!(*str_field, "42");
    /// assert_eq!(str_field.confidence, Confidence::HIGH);
    /// ```
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> ParsedField<U> {
        ParsedField {
            value: f(self.value),
            confidence: self.confidence,
            span: self.span,
            raw: self.raw,
            alternatives: Vec::new(), // Can't map alternatives without Clone bound
        }
    }
}

impl<T> std::ops::Deref for ParsedField<T> {
    type Target = T;

    /// Allows transparent access to the wrapped value.
    ///
    /// # Example
    /// ```
    /// # use sceneforged_parser::model::{ParsedField, Confidence};
    /// let field = ParsedField::certain(42, (0, 2), "42");
    /// assert_eq!(*field, 42);
    /// assert_eq!(field.to_string(), "42"); // Can call i32 methods directly
    /// ```
    fn deref(&self) -> &T {
        &self.value
    }
}

/// Type alias for optional parsed fields.
///
/// This is the common case where a field may or may not be present in the input.
/// If present, it comes with full metadata via `ParsedField<T>`.
pub type OptionalField<T> = Option<ParsedField<T>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_new_clamps_values() {
        assert_eq!(Confidence::new(-0.5).value(), 0.0);
        assert_eq!(Confidence::new(1.5).value(), 1.0);
        assert_eq!(Confidence::new(0.75).value(), 0.75);
    }

    #[test]
    fn confidence_constants() {
        assert_eq!(Confidence::CERTAIN.value(), 1.0);
        assert_eq!(Confidence::HIGH.value(), 0.9);
        assert_eq!(Confidence::MEDIUM.value(), 0.7);
        assert_eq!(Confidence::LOW.value(), 0.5);
        assert_eq!(Confidence::GUESS.value(), 0.3);
    }

    #[test]
    fn confidence_needs_review() {
        assert!(Confidence::GUESS.needs_review());
        assert!(!Confidence::LOW.needs_review()); // Exactly 0.5
        assert!(!Confidence::MEDIUM.needs_review());
        assert!(!Confidence::HIGH.needs_review());
    }

    #[test]
    fn confidence_is_confident() {
        assert!(!Confidence::GUESS.is_confident());
        assert!(!Confidence::LOW.is_confident());
        assert!(Confidence::MEDIUM.is_confident()); // Exactly 0.7
        assert!(Confidence::HIGH.is_confident());
        assert!(Confidence::CERTAIN.is_confident());
    }

    #[test]
    fn parsed_field_creation() {
        let field = ParsedField::new(42, Confidence::HIGH, (5, 7), "42");
        assert_eq!(field.value, 42);
        assert_eq!(field.confidence, Confidence::HIGH);
        assert_eq!(field.span, (5, 7));
        assert_eq!(field.raw, "42");
        assert!(field.alternatives.is_empty());
    }

    #[test]
    fn parsed_field_certain() {
        let field = ParsedField::certain("test", (0, 4), "test");
        assert_eq!(field.confidence, Confidence::CERTAIN);
    }

    #[test]
    fn parsed_field_with_alternatives() {
        let field = ParsedField::new(1920, Confidence::MEDIUM, (0, 4), "1920")
            .with_alternatives(vec![1080, 720]);
        assert_eq!(field.alternatives.len(), 2);
        assert_eq!(field.alternatives[0], 1080);
    }

    #[test]
    fn parsed_field_map() {
        let num = ParsedField::new(42, Confidence::HIGH, (0, 2), "42");
        let string = num.map(|n| format!("The answer is {}", n));
        assert_eq!(*string, "The answer is 42");
        assert_eq!(string.confidence, Confidence::HIGH);
        assert_eq!(string.span, (0, 2));
        assert_eq!(string.raw, "42");
    }

    #[test]
    fn parsed_field_deref() {
        let field = ParsedField::certain(42, (0, 2), "42");
        assert_eq!(*field, 42);
        // Can use methods from the wrapped type
        assert_eq!(field.to_string(), "42");
    }

    #[test]
    fn confidence_comparison() {
        assert!(Confidence::GUESS < Confidence::LOW);
        assert!(Confidence::LOW < Confidence::MEDIUM);
        assert!(Confidence::MEDIUM < Confidence::HIGH);
        assert!(Confidence::HIGH < Confidence::CERTAIN);
    }

    #[test]
    fn confidence_equality() {
        assert_eq!(Confidence::new(0.7), Confidence::MEDIUM);
        assert_eq!(Confidence::new(1.0), Confidence::CERTAIN);
    }
}
