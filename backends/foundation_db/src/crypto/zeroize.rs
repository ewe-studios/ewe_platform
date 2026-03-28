//! Zeroizing utilities for secure memory handling.

use zeroize::Zeroize;

/// A wrapper that zeroizes memory on drop.
///
/// This is useful for sensitive data like encryption keys, tokens, and passwords
/// that should be cleared from memory when no longer needed.
#[derive(Clone)]
pub struct ZeroizingSecret<T: Zeroize + Clone>(T);

impl<T: Zeroize + Clone> ZeroizingSecret<T> {
    /// Create a new zeroizing secret.
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    /// Get a reference to the inner value.
    pub fn get(&self) -> &T {
        &self.0
    }

    /// Get the inner value, consuming the secret.
    pub fn expose(self) -> T {
        let value = self.0.clone();
        drop(self); // Ensure zeroize runs
        value
    }

    /// Map the inner value to a new type.
    pub fn map<U: Zeroize + Clone, F: FnOnce(&T) -> U>(self, f: F) -> ZeroizingSecret<U> {
        let result = ZeroizingSecret::new(f(&self.0));
        drop(self);
        result
    }
}

impl<T: Zeroize + Clone> AsRef<T> for ZeroizingSecret<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: Zeroize + Clone + Default> Default for ZeroizingSecret<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Zeroize + Clone> Drop for ZeroizingSecret<T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl<T: Zeroize + Clone + PartialEq> PartialEq for ZeroizingSecret<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Zeroize + Clone + core::fmt::Debug> core::fmt::Debug for ZeroizingSecret<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ZeroizingSecret")
            .field("type", &std::any::type_name::<T>())
            .finish()
    }
}

/// Type alias for zeroizing strings.
pub type ZeroizingString = ZeroizingSecret<String>;

impl ZeroizingString {
    /// Create a new zeroizing string.
    pub fn from_string(s: String) -> Self {
        Self::new(s)
    }

    /// Get the string reference.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zeroizing_secret() {
        let secret = ZeroizingSecret::new(vec![1u8, 2, 3, 4]);
        assert_eq!(secret.get(), &vec![1u8, 2, 3, 4]);

        let exposed = secret.expose();
        assert_eq!(exposed, vec![1u8, 2, 3, 4]);
    }

    #[test]
    fn test_zeroizing_string() {
        let secret = ZeroizingString::from_string("sensitive data".to_string());
        assert_eq!(secret.as_str(), "sensitive data");
    }
}
