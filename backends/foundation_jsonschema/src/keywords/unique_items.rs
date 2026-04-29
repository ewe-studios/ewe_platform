//! Validates `uniqueItems` — all array items must be distinct.
//!
//! WHY: JSON Schema requires that when `uniqueItems: true`, no two items in
//! the array may be equal. Equality uses JSON Schema semantics (1 == 1.0).

use alloc::boxed::Box;

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
        } else {
            // Find the first duplicate pair
            if let Value::Array(arr) = instance {
                let (first, second) = find_duplicate(arr).unwrap_or((0, 1));
                Err(ValidationErrorBuilder::new(
                    instance_path.materialize(),
                    self.schema_path.clone(),
                )
                .build(ValidationErrorKind::UniqueItems { first, second }))
            } else {
                unreachable!()
            }
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

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        _ => a == b,
    }
}

fn find_duplicate(arr: &[Value]) -> Option<(usize, usize)> {
    for i in 0..arr.len() {
        for j in (i + 1)..arr.len() {
            if values_equal(&arr[i], &arr[j]) {
                return Some((i, j));
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
