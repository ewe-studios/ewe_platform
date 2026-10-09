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

