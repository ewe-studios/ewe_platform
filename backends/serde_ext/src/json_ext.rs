use std::collections::VecDeque;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::{JsonError, JsonResult};

///  Extension trait that allows us define a conversion of a JSON value to
/// more native rust types.
///
/// It's implemented for bool, u32/64, f64, i32/64 and str
pub trait AsType<'a>: Sized {
    fn from_json(value: &'a Value) -> Result<Self, JsonError>;
}

impl<'a> AsType<'a> for &'a str {
    fn from_json(value: &'a Value) -> Result<Self, JsonError> {
        value.as_str().ok_or(JsonError::ValueNotType("str"))
    }
}

impl AsType<'_> for bool {
    fn from_json(value: &Value) -> Result<Self, JsonError> {
        value.as_bool().ok_or(JsonError::ValueNotType("bool"))
    }
}

impl AsType<'_> for f64 {
    fn from_json(value: &Value) -> Result<Self, JsonError> {
        value.as_f64().ok_or(JsonError::ValueNotType("f64"))
    }
}

impl AsType<'_> for u64 {
    fn from_json(value: &Value) -> Result<Self, JsonError> {
        value.as_u64().ok_or(JsonError::ValueNotType("u64"))
    }
}

impl AsType<'_> for u32 {
    fn from_json(value: &Value) -> Result<Self, JsonError> {
        value
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(JsonError::ValueNotType("u32"))
    }
}

impl AsType<'_> for i32 {
    fn from_json(value: &Value) -> Result<Self, JsonError> {
        value
            .as_i64()
            .and_then(|v| i32::try_from(v).ok())
            .ok_or(JsonError::ValueNotType("i32"))
    }
}

impl AsType<'_> for i64 {
    fn from_json(value: &Value) -> Result<Self, JsonError> {
        value.as_i64().ok_or(JsonError::ValueNotType("i64"))
    }
}

pub trait JsonValueExt {
    fn json_new_object() -> Value;

    /// Returns an owned type `T` for a given name or pointer path.
    /// - `name_or_pointer`: Can be a direct name or a pointer path (path starting with `/`),
    fn json_get<T: DeserializeOwned>(&self, name_or_pointer: &str) -> JsonResult<T>;

    /// Returns an reference of type `T` (or value for copy type) for a given name or pointer path.
    /// - `name_or_pointer`: Can be a direct name or a pointer path (path starting with `/`),
    fn json_get_as<'a, T: AsType<'a>>(&'a self, name_or_pointer: &str) -> JsonResult<T>;

    /// Returns an owned type `T` for a given name or pointer path replacing with `Null`.
    /// - `name_or_pointer`: Can be a direct name or a pointer path (path starting with `/`),
    fn json_take<T: DeserializeOwned>(&mut self, name_or_pointer: &str) -> JsonResult<T>;

    /// Inserts a new value of type `T` at the specified name or pointer path.
    /// It creates a missing `Value::Object` entries as needed.
    fn json_insert<T: Serialize>(&mut self, name_or_pointer: &str, value: T) -> JsonResult<()>;

    /// Walks through all properties in the JSON value tree and calls the callback function
    /// on each key.
    /// It creates a missing `Value::Object` entries as needed.
    fn json_walk<F>(&mut self, callback: F) -> bool
    where
        F: FnMut(&mut Map<String, Value>, &str) -> bool;

    /// Returns a pretty-printed string representation of the JSON value.
    fn json_pretty(&self) -> JsonResult<String>;
}

impl JsonValueExt for Value {
    fn json_new_object() -> Value {
        Value::Object(Map::new())
    }

    fn json_get<T: DeserializeOwned>(&self, name_or_pointer: &str) -> JsonResult<T> {
        let value = if name_or_pointer.starts_with('/') {
            self.pointer(name_or_pointer)
                .ok_or_else(|| JsonError::PropertyNotFound(name_or_pointer.to_string()))?
        } else {
            self.get(name_or_pointer)
                .ok_or_else(|| JsonError::PropertyNotFound(name_or_pointer.to_string()))?
        };

        let value: T = serde_json::from_value(value.clone())?;
        Ok(value)
    }

    fn json_get_as<'a, T: AsType<'a>>(&'a self, name_or_pointer: &str) -> JsonResult<T> {
        let value = if name_or_pointer.starts_with('/') {
            self.pointer(name_or_pointer)
                .ok_or_else(|| JsonError::PropertyNotFound(name_or_pointer.to_string()))?
        } else {
            self.get(name_or_pointer)
                .ok_or_else(|| JsonError::PropertyNotFound(name_or_pointer.to_string()))?
        };

        T::from_json(value)
    }

    fn json_take<T: DeserializeOwned>(&mut self, name_or_pointer: &str) -> JsonResult<T> {
        let value = if name_or_pointer.starts_with('/') {
            self.pointer_mut(name_or_pointer)
                .map(Value::take)
                .ok_or_else(|| JsonError::PropertyNotFound(name_or_pointer.to_string()))?
        } else {
            self.get_mut(name_or_pointer)
                .map(Value::take)
                .ok_or_else(|| JsonError::PropertyNotFound(name_or_pointer.to_string()))?
        };

        let value: T = serde_json::from_value(value)?;
        Ok(value)
    }

    fn json_insert<T: Serialize>(&mut self, name_or_pointer: &str, value: T) -> JsonResult<()> {
        let new_value = serde_json::to_value(value)?;

        if !name_or_pointer.starts_with('/') {
            match self {
                Value::Object(map) => {
                    map.insert(name_or_pointer.to_string(), new_value);
                    Ok(())
                }
                _ => Err(JsonError::custom("Value is not an Object, cannot x_insert")),
            }
        } else {
            let parts: Vec<&str> = name_or_pointer.split('/').skip(1).collect();
            let mut current = self;

            // -- Add the eventual missing parents
            for &part in &parts[..parts.len() - 1] {
                match current {
                    Value::Object(map) => {
                        current = map.entry(part).or_insert_with(|| json!({}));
                    }
                    _ => return Err(JsonError::custom("Path does not point to an Object")),
                }
            }

            // -- Set the value at the last element
            if let Some(&last_part) = parts.last() {
                match current {
                    Value::Object(map) => {
                        map.insert(last_part.to_string(), new_value);
                        Ok(())
                    }
                    _ => Err(JsonError::into_custom("Path does not point to an Object")),
                }
            } else {
                Err(JsonError::into_custom("Invalid path"))
            }
        }
    }

    fn json_walk<F>(&mut self, mut callback: F) -> bool
    where
        F: FnMut(&mut Map<String, Value>, &str) -> bool,
    {
        let mut queue = VecDeque::new();
        queue.push_back(self);

        while let Some(current) = queue.pop_front() {
            if let Value::Object(map) = current {
                // Call the callback for each property name in the current map
                for key in map.keys().cloned().collect::<Vec<_>>() {
                    let res = callback(map, &key);
                    if !res {
                        return false;
                    }
                }

                // Add all nested objects and arrays to the queue for further processing
                for value in map.values_mut() {
                    if value.is_object() || value.is_array() {
                        queue.push_back(value);
                    }
                }
            } else if let Value::Array(arr) = current {
                // If current value is an array, add its elements to the queue
                for value in arr.iter_mut() {
                    if value.is_object() || value.is_array() {
                        queue.push_back(value);
                    }
                }
            }
        }
        true
    }

    fn json_pretty(&self) -> JsonResult<String> {
        let content = serde_json::to_string_pretty(self)?;
        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::JsonValueExt;
    use serde_json::json;

    type Result<T> = core::result::Result<T, Box<dyn std::error::Error>>; // For tests.

    #[test]
    fn test_value_insert_ok() -> Result<()> {
        // -- Setup & Fixtures
        let mut value = json!({"tokens": 3});
        let fx_node_value = "hello";

        // -- Exec
        value.json_insert("/happy/word", fx_node_value)?;

        // -- Check
        let actual_value: String = value.json_get("/happy/word")?;
        assert_eq!(actual_value.as_str(), fx_node_value);

        Ok(())
    }
}
