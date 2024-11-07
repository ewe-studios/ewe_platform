use std::collections::VecDeque;

use serde::de::DeserializeOwned;
use serde::Serialize;
use toml::{map::Map, Value};
use toml_datetime::{Date, Datetime};

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
        value.as_float().ok_or(ValueError::ValueNotType("f64"))
    }
}

impl AsType<'_, Value> for u32 {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        value
            .as_integer()
            .map(|val| val.unsigned_abs())
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(ValueError::ValueNotType("u32"))
    }
}

impl AsType<'_, Value> for i32 {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        value
            .as_integer()
            .and_then(|v| i32::try_from(v).ok())
            .ok_or(ValueError::ValueNotType("i32"))
    }
}

impl AsType<'_, Value> for i64 {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        value.as_integer().ok_or(ValueError::ValueNotType("i64"))
    }
}

impl AsType<'_, Value> for Date {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        match value.as_datetime() {
            Some(item) => match item.date {
                Some(item_date) => Ok(item_date),
                None => Err(ValueError::ValueNotType("date")),
            },
            None => Err(ValueError::ValueNotType("date")),
        }
    }
}

impl AsType<'_, Value> for Datetime {
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        value
            .as_datetime()
            .map(|val| val.to_owned())
            .ok_or(ValueError::ValueNotType("datetime"))
    }
}

impl AsType<'_, Value> for u64 {
    /// For u64 we take the integer (i64) value and convert into an unsigned absolute
    /// of the value, so be aware you might loose sign and potential precision.
    fn from_value(value: &Value) -> ValueResult<Self, ValueError> {
        value
            .as_integer()
            .map(|val| val.unsigned_abs())
            .ok_or(ValueError::ValueNotType("u64"))
    }
}

// -- Error and Result

#[derive(Debug, derive_more::From)]
pub enum TomlError {
    #[from(ignore)]
    Value(ValueError),

    #[from(ignore)]
    NotFound,

    #[from(ignore)]
    NotTable,

    #[from(ignore)]
    NoValueAt(String),

    #[from(ignore)]
    FailedDeserialization(toml::de::Error),

    #[from(ignore)]
    FailedSerialization(toml::ser::Error),
}

// -- From<T> and Into<TomlErro>

impl From<toml::ser::Error> for TomlError {
    fn from(value: toml::ser::Error) -> Self {
        Self::FailedSerialization(value)
    }
}

impl From<toml::de::Error> for TomlError {
    fn from(value: toml::de::Error) -> Self {
        Self::FailedDeserialization(value)
    }
}

impl From<ValueError> for TomlError {
    fn from(value: ValueError) -> Self {
        Self::Value(value)
    }
}

// --- region: Custom methods

impl TomlError {
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

// --- end region: Custom methods

// --- region: Error & Display boilerplate

impl std::error::Error for TomlError {}

impl core::fmt::Display for TomlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

// --- end region: Error & Display boilerplate

// --- end region: TomlError

impl PointerValueExt for Value {
    type Item = Value;
    type Error = TomlError;

    fn get_path(&self, name_or_pointer: &str) -> ValueResult<&Self::Item, Self::Error> {
        if !name_or_pointer.contains("/") {
            return self
                .get(name_or_pointer)
                .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()).into());
        }

        let pointer_parts: Vec<&str> = name_or_pointer.split("/").skip(1).collect();

        let mut current_value = self;
        for &part in &pointer_parts[..pointer_parts.len() - 1] {
            match current_value {
                Value::Table(table) => {
                    if !table.contains_key(part) {
                        return Err(TomlError::NotFound);
                    }
                    match table.get(part) {
                        Some(item) => {
                            current_value = item;
                        }
                        None => return Err(TomlError::NoValueAt(String::from(part))),
                    }
                }
                _ => return Err(TomlError::NotTable),
            }
        }

        if let Some(&last_part) = pointer_parts.last() {
            match current_value {
                Value::Table(table) => {
                    if !table.contains_key(last_part) {
                        return Err(TomlError::NotFound);
                    }
                    match table.get(last_part) {
                        Some(item) => {
                            return Ok(item);
                        }
                        None => return Err(TomlError::NoValueAt(String::from(last_part))),
                    }
                }
                _ => return Err(TomlError::NotTable),
            }
        }

        return Err(TomlError::custom("invalid path"));
    }

    fn take_path(&mut self, name_or_pointer: &str) -> ValueResult<Self::Item, Self::Error> {
        if !name_or_pointer.contains("/") {
            return self
                .get(name_or_pointer)
                .take()
                .map(|val| val.to_owned())
                .ok_or_else(|| ValueError::PropertyNotFound(name_or_pointer.to_string()).into());
        }

        let pointer_parts: Vec<&str> = name_or_pointer.split("/").skip(1).collect();

        let mut current_value = self;
        for &part in &pointer_parts[..pointer_parts.len() - 1] {
            match current_value {
                Value::Table(table) => {
                    if !table.contains_key(part) {
                        return Err(TomlError::NotFound);
                    }
                    match table.get_mut(part) {
                        Some(item) => {
                            current_value = item;
                        }
                        None => return Err(TomlError::NoValueAt(String::from(part))),
                    }
                }
                _ => return Err(TomlError::NotTable),
            }
        }

        if let Some(&last_part) = pointer_parts.last() {
            match current_value {
                Value::Table(table) => {
                    if !table.contains_key(last_part) {
                        return Err(TomlError::NotFound);
                    }
                    match table.get_mut(last_part) {
                        Some(item) => {
                            return Ok(item.to_owned());
                        }
                        None => return Err(TomlError::NoValueAt(String::from(last_part))),
                    }
                }
                _ => return Err(TomlError::NotTable),
            }
        }

        return Err(TomlError::custom("invalid path"));
    }
}

pub trait TomlValueExt: DynamicValueExt {
    /// Walks through all properties in the JSON value tree and calls the callback function
    /// on each key.
    /// It creates a missing `Value::Table` entries as needed.
    fn d_walk<F>(&mut self, callback: F) -> bool
    where
        F: FnMut(&mut Map<String, Value>, &str) -> bool;
}

impl DynamicValueExt for Value {
    type Item = Value;
    type Error = TomlError;

    fn d_new() -> Value {
        Value::Table(Map::new())
    }

    fn d_get<T: DeserializeOwned>(&self, name_or_pointer: &str) -> ValueResult<T, Self::Error> {
        let value = PointerValueExt::get_path(self, name_or_pointer)?;
        T::deserialize(value.to_owned()).map_err(|err| err.into())
    }

    fn d_get_as<'a, V: AsType<'a, Self::Item>>(
        &'a self,
        name_or_pointer: &str,
    ) -> ValueResult<V, Self::Error> {
        let value = PointerValueExt::get_path(self, name_or_pointer)?;
        V::from_value(value).map_err(|err| err.into())
    }

    fn d_take<T: DeserializeOwned>(
        &mut self,
        name_or_pointer: &str,
    ) -> ValueResult<T, Self::Error> {
        let value = PointerValueExt::take_path(self, name_or_pointer)?;
        T::deserialize(value.to_owned()).map_err(|err| err.into())
    }

    fn d_insert<T: Serialize>(
        &mut self,
        name_or_pointer: &str,
        value: T,
    ) -> ValueResult<(), Self::Error> {
        let new_value = Value::try_from(value)?;

        if !name_or_pointer.starts_with('/') {
            match self {
                Value::Table(map) => {
                    map.insert(name_or_pointer.to_string(), new_value);
                    Ok(())
                }
                _ => Err(TomlError::custom("Value is not an Table, cannot d_insert")),
            }
        } else {
            let parts: Vec<&str> = name_or_pointer.split('/').skip(1).collect();
            let mut current = self;

            // -- Add the eventual missing parents
            for &part in &parts[..parts.len() - 1] {
                match current {
                    Value::Table(table) => {
                        current = table
                            .entry(part)
                            .or_insert_with(|| toml::Value::Table(Map::new()));
                    }
                    _ => return Err(TomlError::custom("Path does not point to an Table")),
                }
            }

            // -- Set the value at the last element
            if let Some(&last_part) = parts.last() {
                match current {
                    Value::Table(map) => {
                        map.insert(last_part.to_string(), new_value);
                        Ok(())
                    }
                    _ => Err(TomlError::into_custom("Path does not point to an Table")),
                }
            } else {
                Err(TomlError::into_custom("Invalid path"))
            }
        }
    }

    fn d_pretty(&self) -> ValueResult<String, Self::Error> {
        let content = toml::to_string_pretty(self)?;
        Ok(content)
    }
}

impl TomlValueExt for Value {
    fn d_walk<F>(&mut self, mut callback: F) -> bool
    where
        F: FnMut(&mut Map<String, Value>, &str) -> bool,
    {
        let mut queue = VecDeque::new();
        queue.push_back(self);

        while let Some(current) = queue.pop_front() {
            if let Value::Table(map) = current {
                // Call the callback for each property name in the current map
                for key in map.keys().cloned().collect::<Vec<_>>() {
                    let res = callback(map, &key);
                    if !res {
                        return false;
                    }
                }

                // Add all nested Tables and arrays to the queue for further processing
                for (_, value) in map.iter_mut() {
                    if value.is_table() || value.is_array() {
                        queue.push_back(value);
                    }
                }
            } else if let Value::Array(arr) = current {
                // If current value is an array, add its elements to the queue
                for value in arr.iter_mut() {
                    if value.is_table() || value.is_array() {
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
    use super::{DynamicValueExt, TomlValueExt};
    use toml::toml;

    type Result<T> = core::result::Result<T, Box<dyn std::error::Error>>; // For tests.

    #[test]
    fn test_value_can_walk() -> Result<()> {
        // -- Setup & Fixtures
        let mut value = toml::Value::Table(toml! {
            token=3

            [hello]
            word = "hello"

            [hello.wreckage]
            where = "londo"
        });

        // -- Exec
        assert!(value.d_walk(|tree, key| {
            assert!(tree.contains_key(key));
            true
        }));
        Ok(())
    }

    #[test]
    fn test_value_can_take() -> Result<()> {
        // -- Setup & Fixtures
        let mut value = toml::Value::Table(toml! {
            token=3

            [hello]
            word = "hello"
        });

        // -- Exec
        let content: String = value.d_take("/hello/word")?;
        assert_eq!(&content, "hello");

        // Should
        assert!(matches!(value.d_get::<String>("hello"), Err(_)));
        assert!(matches!(value.d_get::<String>("hello/word"), Err(_)));
        Ok(())
    }

    #[test]
    fn test_value_insert_ok() -> Result<()> {
        // -- Setup & Fixtures
        let mut value = toml::Value::Table(toml! {
            token=3

            [hello]
            word = "hello"
        });

        let fx_node_value = "hello";

        // -- Exec
        let result = value.d_insert("/happy/word", fx_node_value);
        assert!(matches!(result, Ok(())));

        // -- Check
        let actual_value: String = value.d_get("/happy/word")?;
        assert_eq!(actual_value.as_str(), fx_node_value);

        Ok(())
    }
}
