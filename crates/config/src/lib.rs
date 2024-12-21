use derive_more::derive::From;
use serde::de::DeserializeOwned;

#[derive(Debug, From)]
pub enum ConfigError {
    #[from(ignore)]
    IOError(std::io::Error),

    #[from(ignore)]
    DeserializationFailed(toml::de::Error),

    InvalidPath(std::path::PathBuf),
}

impl From<toml::de::Error> for ConfigError {
    fn from(value: toml::de::Error) -> Self {
        Self::DeserializationFailed(value)
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(value: std::io::Error) -> Self {
        Self::IOError(value)
    }
}

impl std::error::Error for ConfigError {}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

/// value_from_path returns the regular `toml::Value` object which implements the
/// `serde::DeserializeOwned` trait which allows you to directly manipulate the value object
/// instead of a defined type.
pub fn value_from_path<V: Into<std::path::PathBuf>>(target: V) -> ConfigResult<toml::Value> {
    from_path(target)
}

pub fn from_path<T, V>(target: V) -> ConfigResult<T>
where
    T: DeserializeOwned,
    V: Into<std::path::PathBuf>,
{
    let target_path = target.into();
    let config_content = std::fs::read_to_string(target_path)?;
    let config_obj: T = toml::from_str(&config_content)?;
    Ok(config_obj)
}
