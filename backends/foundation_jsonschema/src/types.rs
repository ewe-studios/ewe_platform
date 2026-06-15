//! JSON Schema type representation.
//!
//! WHY: JSON Schema defines 7 primitive types (null, boolean, integer, number,
//! string, array, object). These types must be mapped from `serde_json::Value`
//! and collected into sets for the `type` keyword validator.
//!
//! WHAT: `JsonType` enum for single types, `JsonTypeSet` bitset for type sets.
//!
//! HOW: `JsonTypeSet` uses a u8 bitset (7 types fit in 7 bits) for O(1)
//! membership testing without heap allocation.

use alloc::vec::Vec;
use core::fmt;

use serde_json::Value;

/// The 7 primitive types defined by JSON Schema.
///
/// WHY: JSON Schema has its own type system that differs from JSON —
/// notably, "integer" is a separate type from "number", and types are
/// compared during validation.
///
/// WHAT: One variant per JSON Schema type.
///
/// HOW: Mapped from `serde_json::Value` via `JsonType::of()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum JsonType {
    /// JSON `null` value.
    Null,
    /// JSON `true` or `false`.
    Boolean,
    /// JSON integer (no fractional part).
    Integer,
    /// JSON number with fractional part.
    Number,
    /// JSON string.
    String,
    /// JSON array.
    Array,
    /// JSON object.
    Object,
}

impl JsonType {
    /// Determine the JSON Schema type of a `serde_json::Value`.
    ///
    /// WHY: Validation needs to compare instance types against schema `type` constraints.
    ///
    /// WHAT: Returns the `JsonType` corresponding to the value's kind.
    ///
    /// HOW: Pattern matches on `Value` variants. For numbers, distinguishes
    /// integer vs. float by checking `is_i64()`/`is_u64()`.
    ///
    /// NOTE: JSON Schema considers integers a subset of numbers — `JsonType::Integer`
    /// values satisfy both `"type": "integer"` and `"type": "number"`.
    #[must_use]
    pub fn of(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(_) => Self::Boolean,
            Value::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    Self::Integer
                } else if let Some(f) = n.as_f64() {
                    if f.fract() == 0.0 {
                        Self::Integer
                    } else {
                        Self::Number
                    }
                } else {
                    Self::Number
                }
            }
            Value::String(_) => Self::String,
            Value::Array(_) => Self::Array,
            Value::Object(_) => Self::Object,
        }
    }

    /// Return the JSON Schema string name for this type.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::String => "string",
            Self::Array => "array",
            Self::Object => "object",
        }
    }
}

impl fmt::Display for JsonType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Compact bitset holding a set of `JsonType` values.
///
/// WHY: The `type` keyword in JSON Schema accepts either a single type or an
/// array of types. A bitset provides O(1) membership testing without heap
/// allocation — 7 types fit in 7 bits.
///
/// WHAT: A u8 internally with one bit per JSON type.
///
/// HOW: Each `JsonType` variant maps to a bit position via `as u8`. Bitwise
/// operations handle insert, contains, and iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonTypeSet(u8);

impl JsonTypeSet {
    /// Create an empty type set.
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Create a type set containing a single type.
    #[must_use]
    pub const fn with(ty: JsonType) -> Self {
        Self(1 << (ty as u8))
    }

    /// Insert a type into the set.
    pub fn insert(&mut self, ty: JsonType) {
        self.0 |= 1 << (ty as u8);
    }

    /// Check if the set contains a type.
    ///
    /// NOTE: If this set contains `Integer`, `contains(Number)` also returns
    /// `true` (since integer is a subset of number in JSON Schema).
    #[must_use]
    pub fn contains(&self, ty: JsonType) -> bool {
        (self.0 & (1 << (ty as u8))) != 0
    }

    /// Returns `true` if the set contains no types.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Returns the number of types in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.count_ones() as usize
    }

    /// Return an iterator over the types in the set.
    #[must_use]
    pub fn iter(&self) -> JsonTypeSetIter {
        JsonTypeSetIter {
            set: self.0,
            pos: 0,
        }
    }

    /// Parse a JSON Schema `type` value (string or array of strings) into a `JsonTypeSet`.
    ///
    /// WHY: The `type` keyword can be a single string (`"string"`) or an array
    /// (`["string", "number"]`). This handles both forms.
    ///
    /// WHAT: Returns a `JsonTypeSet` matching the declared types.
    ///
    /// HOW: Pattern matches on `Value::String` vs `Value::Array`. Returns
    /// `None` for unrecognized type names.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn from_schema_value(value: &Value) -> Option<Self> {
        match value {
            Value::String(s) => parse_type_name(s).map(JsonTypeSet::with),
            Value::Array(items) => {
                let mut set = Self::new();
                for item in items {
                    if let Value::String(s) = item {
                        set.insert(parse_type_name(s)?);
                    } else {
                        return None;
                    }
                }
                Some(set)
            }
            _ => None,
        }
    }
}

impl Default for JsonTypeSet {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoIterator for &JsonTypeSet {
    type Item = JsonType;
    type IntoIter = JsonTypeSetIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl fmt::Display for JsonTypeSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let types: Vec<&str> = self.iter().map(|t| t.as_str()).collect();
        match types.len() {
            0 => write!(f, "(empty)"),
            1 => write!(f, "{}", types[0]),
            _ => write!(f, "({})", types.join(", ")),
        }
    }
}

/// Iterator over the types in a `JsonTypeSet`.
pub struct JsonTypeSetIter {
    set: u8,
    pos: u8,
}

impl Iterator for JsonTypeSetIter {
    type Item = JsonType;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < 7 {
            let bit = 1u8 << self.pos;
            self.pos += 1;
            if (self.set & bit) != 0 {
                return match self.pos {
                    1 => Some(JsonType::Null),
                    2 => Some(JsonType::Boolean),
                    3 => Some(JsonType::Integer),
                    4 => Some(JsonType::Number),
                    5 => Some(JsonType::String),
                    6 => Some(JsonType::Array),
                    7 => Some(JsonType::Object),
                    _ => None,
                };
            }
        }
        None
    }
}

/// Parse a JSON Schema type name string into a `JsonType`.
fn parse_type_name(s: &str) -> Option<JsonType> {
    match s {
        "null" => Some(JsonType::Null),
        "boolean" => Some(JsonType::Boolean),
        "integer" => Some(JsonType::Integer),
        "number" => Some(JsonType::Number),
        "string" => Some(JsonType::String),
        "array" => Some(JsonType::Array),
        "object" => Some(JsonType::Object),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_type_of_null() {
        assert_eq!(JsonType::of(&Value::Null), JsonType::Null);
    }

    #[test]
    fn test_json_type_of_boolean() {
        assert_eq!(JsonType::of(&Value::Bool(true)), JsonType::Boolean);
        assert_eq!(JsonType::of(&Value::Bool(false)), JsonType::Boolean);
    }

    #[test]
    fn test_json_type_of_integer() {
        assert_eq!(JsonType::of(&Value::Number(42.into())), JsonType::Integer);
    }

    #[test]
    fn test_json_type_of_number() {
        let n: Value = serde_json::Number::from_f64(std::f64::consts::PI)
            .unwrap()
            .into();
        assert_eq!(JsonType::of(&n), JsonType::Number);
    }

    #[test]
    fn test_json_type_of_string() {
        assert_eq!(
            JsonType::of(&Value::String("hello".into())),
            JsonType::String
        );
    }

    #[test]
    fn test_json_type_of_array() {
        assert_eq!(JsonType::of(&Value::Array(vec![])), JsonType::Array);
    }

    #[test]
    fn test_json_type_of_object() {
        assert_eq!(
            JsonType::of(&Value::Object(Default::default())),
            JsonType::Object
        );
    }

    #[test]
    fn test_json_type_set_insert_and_contains() {
        let mut set = JsonTypeSet::new();
        assert!(set.is_empty());
        set.insert(JsonType::String);
        set.insert(JsonType::Integer);
        assert_eq!(set.len(), 2);
        assert!(set.contains(JsonType::String));
        assert!(set.contains(JsonType::Integer));
        assert!(!set.contains(JsonType::Number));
    }

    #[test]
    fn test_json_type_set_with() {
        let set = JsonTypeSet::with(JsonType::Boolean);
        assert_eq!(set.len(), 1);
        assert!(set.contains(JsonType::Boolean));
    }

    #[test]
    fn test_json_type_set_iter() {
        let mut set = JsonTypeSet::new();
        set.insert(JsonType::Null);
        set.insert(JsonType::String);
        let types: Vec<JsonType> = set.iter().collect();
        assert_eq!(types, vec![JsonType::Null, JsonType::String]);
    }

    #[test]
    fn test_json_type_set_from_schema_value_string() {
        let set = JsonTypeSet::from_schema_value(&Value::String("string".into())).unwrap();
        assert!(set.contains(JsonType::String));
    }

    #[test]
    fn test_json_type_set_from_schema_value_array() {
        let value = Value::Array(vec![
            Value::String("string".into()),
            Value::String("number".into()),
        ]);
        let set = JsonTypeSet::from_schema_value(&value).unwrap();
        assert!(set.contains(JsonType::String));
        assert!(set.contains(JsonType::Number));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_json_type_set_from_schema_value_invalid() {
        assert!(JsonTypeSet::from_schema_value(&Value::Number(42.into())).is_none());
    }

    #[test]
    fn test_json_type_display() {
        assert_eq!(format!("{}", JsonType::String), "string");
        assert_eq!(format!("{}", JsonType::Number), "number");
    }

    #[test]
    fn test_json_type_set_display() {
        let mut set = JsonTypeSet::new();
        set.insert(JsonType::Null);
        set.insert(JsonType::String);
        assert_eq!(format!("{set}"), "(null, string)");
    }
}
