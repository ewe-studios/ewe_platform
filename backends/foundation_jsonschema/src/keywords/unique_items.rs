//! Validates `uniqueItems` — all array items must be distinct.
//!
//! WHY: JSON Schema requires that when `uniqueItems: true`, no two items in
//! the array may be equal. Equality uses JSON Schema semantics (1 == 1.0).

use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::string::String;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct UniqueItemsValidator {
    schema_path: Location,
}

impl UniqueItemsValidator {
    pub fn new(schema_path: Location) -> Self {
        Self { schema_path }
    }
}

impl Validate for UniqueItemsValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::Array(arr) = instance {
            is_unique(arr)
        } else {
            true
        }
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        if self.is_valid(instance, ctx) {
            Ok(())
        } else if let Value::Array(arr) = instance {
            let (first, second) = find_duplicate(arr).unwrap_or((0, 1));
            Err(
                ValidationErrorBuilder::new(instance_path.materialize(), self.schema_path.clone())
                    .build(ValidationErrorKind::UniqueItems { first, second }),
            )
        } else {
            unreachable!()
        }
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        match self.validate(instance, instance_path, ctx) {
            Ok(()) => Box::new(core::iter::empty()),
            Err(e) => Box::new(core::iter::once(e)),
        }
    }
}

/// JSON Schema equality: 1 == 1.0, but large integers use exact comparison.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => numbers_equal(a, b),
        _ => a == b,
    }
}

/// Compare two JSON numbers with correct integer precision.
fn numbers_equal(a: &serde_json::Number, b: &serde_json::Number) -> bool {
    // Same-representation integers: exact comparison.
    if a.as_i64().is_some() && b.as_i64().is_some() {
        return a.as_i64() == b.as_i64();
    }
    if a.as_u64().is_some() && b.as_u64().is_some() {
        return a.as_u64() == b.as_u64();
    }
    // Both are floats (not representable as i64/u64): exact f64 comparison.
    if a.as_f64().is_some()
        && b.as_f64().is_some()
        && a.as_i64().is_none()
        && a.as_u64().is_none()
        && b.as_i64().is_none()
        && b.as_u64().is_none()
    {
        return a.as_f64() == b.as_f64();
    }
    // Cross-representation: compare as f64 (handles 0 == 0.0).
    a.as_f64() == b.as_f64()
}

/// Canonical string representation for hashing.
/// Uses `serde_json` to serialize, which gives consistent output for JSON values.
fn value_hash(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(b) => alloc::format!("bool:{b}"),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                alloc::format!("num:{i}")
            } else if let Some(u) = n.as_u64() {
                alloc::format!("num:{u}")
            } else {
                alloc::format!("num:{}", n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => alloc::format!("str:{s}"),
        Value::Array(arr) => {
            let inner: Vec<String> = arr.iter().map(value_hash).collect();
            alloc::format!("arr:[{}]", inner.join(","))
        }
        Value::Object(obj) => {
            let inner: Vec<String> = obj
                .iter()
                .map(|(k, v)| alloc::format!("{k}={}", value_hash(v)))
                .collect();
            alloc::format!("obj:[{}]", inner.join(","))
        }
    }
}

fn find_duplicate(arr: &[Value]) -> Option<(usize, usize)> {
    // Use hash-based detection for O(n) performance on large arrays.
    // Fall back to O(n^2) for small arrays to avoid allocation overhead.
    if arr.len() <= 64 {
        for i in 0..arr.len() {
            for j in (i + 1)..arr.len() {
                if values_equal(&arr[i], &arr[j]) {
                    return Some((i, j));
                }
            }
        }
    } else {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for (i, item) in arr.iter().enumerate() {
            let h = value_hash(item);
            if !seen.insert(h) {
                // Hash collision — verify with exact comparison.
                if let Some((j, _)) = arr
                    .iter()
                    .take(i)
                    .enumerate()
                    .find(|(_, prev)| values_equal(prev, item))
                {
                    return Some((j, i));
                }
            }
        }
    }
    None
}

fn is_unique(arr: &[Value]) -> bool {
    find_duplicate(arr).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> ValidationContext {
        ValidationContext::new()
    }

    #[test]
    fn all_unique() {
        let v = UniqueItemsValidator::new(Location::new());
        assert!(v.is_valid(&json!([1, 2, 3]), &mut ctx()));
    }

    #[test]
    fn has_duplicate() {
        let v = UniqueItemsValidator::new(Location::new());
        assert!(!v.is_valid(&json!([1, 2, 1]), &mut ctx()));
    }

    #[test]
    fn int_float_duplicate() {
        let v = UniqueItemsValidator::new(Location::new());
        assert!(!v.is_valid(&json!([1, 1.0]), &mut ctx()));
    }

    #[test]
    fn empty_array() {
        let v = UniqueItemsValidator::new(Location::new());
        assert!(v.is_valid(&json!([]), &mut ctx()));
    }

    #[test]
    fn non_array_always_valid() {
        let v = UniqueItemsValidator::new(Location::new());
        assert!(v.is_valid(&json!("hello"), &mut ctx()));
    }
}
