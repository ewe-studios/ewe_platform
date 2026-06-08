use cedar_policy::{Decision, PolicyId, Response};

pub struct CedarResponse {
    inner: Response,
}

impl CedarResponse {
    pub(crate) fn from_response(response: Response) -> Self {
        Self { inner: response }
    }

    #[must_use]
    pub fn is_allowed(&self) -> bool {
        self.inner.decision() == Decision::Allow
    }

    #[must_use]
    pub fn decision(&self) -> Decision {
        self.inner.decision()
    }

    #[must_use]
    pub fn reasons(&self) -> Vec<PolicyId> {
        self.inner.diagnostics().reason().cloned().collect()
    }

    #[must_use]
    pub fn errors(&self) -> Vec<String> {
        self.inner
            .diagnostics()
            .errors()
            .map(|e| e.to_string())
            .collect()
    }
}

impl core::fmt::Debug for CedarResponse {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CedarResponse")
            .field("decision", &self.decision())
            .field("reasons", &self.reasons())
            .field("errors", &self.errors())
            .finish()
    }
}
