use std::collections::VecDeque;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Map, Value};

use super::{AsType, DynamicValueExt, PointerValueExt, ValueError, ValueResult};

// -- Implement AsType

impl AsType<'_, Value> for String {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        Ok(String::from(
            value.as_str().ok_or(ValueError::ValueNotType("str"))?,
        ))
    }
}

impl<'a> AsType<'a, Value> for &'a str {
    fn from_value(value: &'a Value) -> ValueResult<Self, ValueError> {
        value.as_str().ok_or(ValueError::ValueNotType("str"))
    }
}

impl AsType<'_, Value> for bool {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        value.as_bool().ok_or(ValueError::ValueNotType("bool"))
    }
}

impl AsType<'_, Value> for f64 {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        value.as_f64().ok_or(ValueError::ValueNotType("f64"))
    }
}

impl AsType<'_, Value> for u64 {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        value.as_u64().ok_or(ValueError::ValueNotType("u64"))
    }
}

impl AsType<'_, Value> for u32 {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        value
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(ValueError::ValueNotType("u32"))
    }
}

impl AsType<'_, Value> for i32 {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        value
            .as_i64()
            .and_then(|v| i32::try_from(v).ok())
            .ok_or(ValueError::ValueNotType("i32"))
    }
}

impl AsType<'_, Value> for i64 {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        value.as_i64().ok_or(ValueError::ValueNotType("i64"))
    }
}

// -- Error and Result

#[derive(Debug, derive_more::From)]
pub enum JsonError {
    #[from(ignore)]
    Value(ValueError),

    #[from]
    SerdeJSON(serde_json::Error),
}

// --- region: Custom methods

impl JsonError {
    pub fn value<V: Into<ValueError>>(val: V) -> Self {
        Self::Value(val.into())
    }

    pub fn custom<T>(val: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Value(ValueError::Custom(val.to_string()))
    }

    pub fn into_custom<T>(val: T) -> Self
    where
        T: Into<String>,
    {
        Self::Value(ValueError::Custom(val.into()))
    }
}

impl From<ValueError> for JsonError {
    fn from(value: ValueError) -> Self {
        Self::Value(value)
    }
}

// --- end region: Custom methods

// --- region: Error & Display boilerplate

impl std::error::Error for JsonError {}

impl core::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

// --- end region: Error & Display boilerplate

// --- end region: JsonError
//
pub trait JsonValueExt: DynamicValueExt {
    // type Item;
    // type Error;

    /// Walks through all properties in the JSON value tree and calls the callback function
    /// on each key.
    /// It creates a missing `Value::Object` entries as needed.
    fn d_walk<F>(&mut self, callback: F) -> bool
    where
        F: FnMut(&mut Map<String, Value>, &str) -> bool;
}

impl PointerValueExt for Value {
    type Item = Value;
    type Error = JsonError;

    fn get_path(&self, name_or_pointer: &str) -> ValueResult<&Self::Item, Self::Error> {
        if name_or_pointer.starts_with('/') {
            return self
                .pointer(name_or_pointer)
                .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()).into());
        }
        self.get(name_or_pointer)
            .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()).into())
    }

    fn take_path(&mut self, name_or_pointer: &str) -> ValueResult<Self::Item, Self::Error> {
        if name_or_pointer.starts_with('/') {
            return self
                .pointer_mut(name_or_pointer)
                .map(Value::take)
                .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()).into());
        }
        self.get_mut(name_or_pointer)
            .map(Value::take)
            .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()).into())
    }
}

impl DynamicValueExt for Value {
    type Item = Value;
    type Error = JsonError;

    fn d_new() -> Value {
        Value::Object(Map::new())
    }

    fn d_get<T: DeserializeOwned>(&self, name_or_pointer: &str) -> ValueResult<T, Self::Error> {
        let value = if name_or_pointer.starts_with('/') {
            self.pointer(name_or_pointer)
                .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()))?
        } else {
            self.get(name_or_pointer)
                .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()))?
        };

        let value: T = serde_json::from_value(value.clone())?;
        Ok(value)
    }

    fn d_get_as<'a, V: AsType<'a, Self::Item>>(
        &'a self,
        name_or_pointer: &str,
    ) -> ValueResult<V, Self::Error> {
        let value = if name_or_pointer.starts_with('/') {
            self.pointer(name_or_pointer)
                .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()))?
        } else {
            self.get(name_or_pointer)
                .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()))?
        };

        V::from_value(value).map_err(std::convert::Into::into)
    }

    fn d_take<T: DeserializeOwned>(
        &mut self,
        name_or_pointer: &str,
    ) -> ValueResult<T, Self::Error> {
        let value = if name_or_pointer.starts_with('/') {
            self.pointer_mut(name_or_pointer)
                .map(Value::take)
                .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()))?
        } else {
            self.get_mut(name_or_pointer)
                .map(Value::take)
                .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()))?
        };

        let value: T = serde_json::from_value(value)?;
        Ok(value)
    }

    fn d_insert<T: Serialize>(
        &mut self,
        name_or_pointer: &str,
        value: T,
    ) -> ValueResult<(), Self::Error> {
        let new_value = serde_json::to_value(value)?;

        if name_or_pointer.starts_with('/') {
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
        } else {
            match self {
                Value::Object(map) => {
                    map.insert(name_or_pointer.to_string(), new_value);
                    Ok(())
                }
                _ => Err(JsonError::custom("Value is not an Object, cannot x_insert")),
            }
        }
    }

    fn d_pretty(&self) -> ValueResult<String, Self::Error> {
        let content = serde_json::to_string_pretty(self)?;
        Ok(content)
    }
}

impl JsonValueExt for Value {
    fn d_walk<F>(&mut self, mut callback: F) -> bool
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
}

#[cfg(test)]
mod tests {
    use super::DynamicValueExt;
    use super::JsonValueExt;
    use serde_json::json;

    type Result<T> = core::result::Result<T, Box<dyn std::error::Error>>; // For tests.

    #[test]
    fn test_value_can_walk() {
        // -- Setup & Fixtures
        let mut value = json!({"tokens": 3, "hello": {"word": "hello"}});

        // -- Exec
        assert!(value.d_walk(|tree, key| {
            assert!(tree.contains_key(key));
            true
        }));
    }

    #[test]
    fn test_value_insert_ok() {
        // -- Setup & Fixtures
        let mut value = json!({"tokens": 3});
        let fx_node_value = "hello";

        // -- Exec
        value.d_insert("/happy/word", fx_node_value).unwrap();

        // -- Check
        let actual_value: String = value.d_get("/happy/word").unwrap();
        dbg!(&actual_value);

        assert_eq!(actual_value.as_str(), fx_node_value);
    }

    #[test]
    fn test_value_can_take() {
        // -- Setup & Fixtures
        let mut value = json!({"tokens": 3, "hello": {"word": "hello"}});

        // -- Exec
        let content: String = value.d_take("/hello/word").unwrap();
        assert_eq!(&content, "hello");

        // Should
        assert!(value.d_get::<String>("hello").is_err());
        assert!(value.d_get::<String>("hello/word").is_err());
    }
}
