/// Labels and label-based routing for the IPC bus.
///
/// Each endpoint joins a bus with a label. Messages are routed to endpoints
/// whose labels match the message's selector label expression.

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// A label is a string identifier for an endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct Label(pub String);

impl Label {
    /// Create a new label.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the label string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<S: Into<String>> From<S> for Label {
    fn from(s: S) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Logical expression for label matching.
///
/// Supports boolean combinations: `Leaf`, `Not`, `And`, `Or`, plus `True`/`False` constants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum LabelOp {
    /// Always matches.
    True,
    /// Never matches.
    False,
    /// Matches a specific label string.
    Leaf(String),
    /// Negation.
    Not(Box<LabelOp>),
    /// Conjunction (both must match).
    And(Box<LabelOp>, Box<LabelOp>),
    /// Disjunction (either must match).
    Or(Box<LabelOp>, Box<LabelOp>),
}

impl LabelOp {
    /// Evaluate this label expression against an endpoint label.
    pub fn matches(&self, label: &str) -> bool {
        match self {
            LabelOp::True => true,
            LabelOp::False => false,
            LabelOp::Leaf(s) => label == s,
            LabelOp::Not(sub) => !sub.matches(label),
            LabelOp::And(left, right) => left.matches(label) && right.matches(label),
            LabelOp::Or(left, right) => left.matches(label) || right.matches(label),
        }
    }

    /// Check if this expression matches a given label.
    pub fn validate(&self, label: &Label) -> bool {
        self.matches(&label.0)
    }

    pub fn and(self, v: impl Into<Self>) -> Self {
        Self::And(Box::new(self), Box::new(v.into()))
    }

    pub fn or(self, v: impl Into<Self>) -> Self {
        Self::Or(Box::new(self), Box::new(v.into()))
    }
}

impl std::ops::Not for LabelOp {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::Not(Box::new(self))
    }
}

impl<T: Into<String>> From<T> for LabelOp {
    fn from(value: T) -> Self {
        LabelOp::Leaf(value.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaf_matches() {
        assert!(LabelOp::Leaf("foo".into()).matches("foo"));
        assert!(!LabelOp::Leaf("foo".into()).matches("bar"));
    }

    #[test]
    fn true_false() {
        assert!(LabelOp::True.matches("anything"));
        assert!(!LabelOp::False.matches("anything"));
    }

    #[test]
    fn not() {
        let expr = LabelOp::Not(Box::new(LabelOp::Leaf("foo".into())));
        assert!(!expr.matches("foo"));
        assert!(expr.matches("bar"));
    }

    #[test]
    fn or() {
        let expr = LabelOp::Or(
            Box::new(LabelOp::Leaf("a".into())),
            Box::new(LabelOp::Leaf("b".into())),
        );
        assert!(expr.matches("a"));
        assert!(expr.matches("b"));
        assert!(!expr.matches("c"));
    }

    #[test]
    fn label_new() {
        let label = Label::new("test");
        assert_eq!(label.as_str(), "test");
    }

    #[test]
    fn label_from_string() {
        let label: Label = "hello".into();
        assert_eq!(label.as_str(), "hello");
    }

    #[test]
    fn label_display() {
        let label = Label::new("test-label");
        assert_eq!(format!("{label}"), "test-label");
    }

    #[test]
    fn builder_and() {
        let op = LabelOp::from("a").and("b");
        assert!(!op.matches("a"));
        assert!(!op.matches("b"));
    }

    #[test]
    fn builder_or() {
        let op = LabelOp::from("a").or("b");
        assert!(op.matches("a"));
        assert!(op.matches("b"));
        assert!(!op.matches("c"));
    }

    #[test]
    fn builder_not() {
        let op = !LabelOp::from("a");
        assert!(!op.matches("a"));
        assert!(op.matches("b"));
    }

    #[test]
    fn builder_chain() {
        let op = LabelOp::from("x").or("y").and(!LabelOp::from("z"));
        assert!(op.matches("x"));
        assert!(op.matches("y"));
        assert!(!op.matches("z"));
    }
}
