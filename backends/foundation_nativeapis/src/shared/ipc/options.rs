/// Options for joining an IPC bus.

use super::Label;

/// Configuration for joining a message bus.
#[derive(Clone, Debug)]
pub struct Options {
    /// Bus identifier — unique name all endpoints on the same bus share.
    pub identifier: String,
    /// Endpoint label — used for routing.
    pub label: Label,
    /// Authentication token — optional. If set, endpoints must share the same token.
    pub token: String,
    /// Whether to become the bus controller if none exists.
    pub controller_affinity: bool,
}

impl Options {
    pub fn new(identifier: impl Into<String>, label: Label) -> Self {
        Self {
            identifier: identifier.into(),
            label,
            token: String::new(),
            controller_affinity: false,
        }
    }

    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = token.into();
        self
    }

    pub fn controller_affinity(mut self, affinity: bool) -> Self {
        self.controller_affinity = affinity;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_new() {
        let opts = Options::new("test-bus", Label::new("my-endpoint"));
        assert_eq!(opts.identifier, "test-bus");
        assert_eq!(opts.label.as_str(), "my-endpoint");
        assert!(opts.token.is_empty());
        assert!(!opts.controller_affinity);
    }

    #[test]
    fn options_builder() {
        let opts = Options::new("test-bus", Label::new("my-endpoint"))
            .token("secret")
            .controller_affinity(true);
        assert_eq!(opts.token, "secret");
        assert!(opts.controller_affinity);
    }
}
