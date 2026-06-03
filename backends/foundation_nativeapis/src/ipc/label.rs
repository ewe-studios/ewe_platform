/// Labels and label-based routing for the IPC bus.
///
/// Each endpoint joins a bus with a label. Messages are routed to endpoints
/// whose labels match the message's selector label expression.

/// A label is a string identifier for an endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

    /// Check if this expression matches a given label (alias for `matches`).
    pub fn evaluate(&self, label: &Label) -> bool {
        self.matches(&label.0)
    }
}

/// Create a `LabelOp` from a label string (convenience constructor for `Leaf`).
#[macro_export]
macro_rules! label {
    ($s:expr) => {
        $crate::ipc::LabelOp::Leaf($s.into())
    };
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
    fn and() {
        let expr = LabelOp::And(
            Box::new(LabelOp::Leaf("a".into())),
            Box::new(LabelOp::Leaf("b".into())),
        );
        assert!(!expr.matches("a"));
        assert!(!expr.matches("b"));
        // And requires BOTH to match the SAME label, so "a" != "b" — always false
        // This is correct: a single label can't be both "a" and "b"
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
    fn complex_expression() {
        // "a" OR ("b" AND NOT "c")
        let expr = LabelOp::Or(
            Box::new(LabelOp::Leaf("a".into())),
            Box::new(LabelOp::And(
                Box::new(LabelOp::Leaf("b".into())),
                Box::new(LabelOp::Not(Box::new(LabelOp::Leaf("c".into())))),
            )),
        );
        assert!(expr.matches("a"));
        assert!(expr.matches("b"));  // b matches, and b != c, so NOT "c" is true
        assert!(!expr.matches("c"));  // c doesn't match "a", and "b" fails
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
}
