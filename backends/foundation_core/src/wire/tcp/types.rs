/// Endpoint represents a target endpoint to be connected
/// to communication.
#[derive(Clone, Debug)]
pub enum Endpoint<I: Clone> {
    Plain(url::Url),
    Encrypted(url::Url),
    EncryptedWithIdentity(url::Url, I),
}

#[allow(unused)]
impl Endpoint<()> {
    #[inline]
    pub fn plain(target: url::Url) -> Self {
        Endpoint::Plain(target)
    }

    #[inline]
    pub fn encrypted(target: url::Url) -> Self {
        Endpoint::Encrypted(target)
    }
}

#[allow(unused)]
impl<T: Clone> Endpoint<T> {
    #[inline]
    pub fn encrypted_with_identity(target: url::Url, identity: T) -> Self {
        Endpoint::EncryptedWithIdentity(target, identity)
    }
}

// --- Custom methods / Helper methods

#[allow(unused)]
impl<T: Clone> Endpoint<T> {
    /// Returns a copy of the url of the target endpont.
    #[inline]
    pub fn url(&self) -> url::Url {
        match self {
            Self::Plain(inner) => inner.clone(),
            Self::Encrypted(inner) => inner.clone(),
            Self::EncryptedWithIdentity(inner, _) => inner.clone(),
        }
    }

    #[inline]
    pub fn host(&self) -> String {
        return match self {
            Self::Plain(inner) => self.get_host_from(&inner),
            Self::Encrypted(inner) => self.get_host_from(&inner),
            Self::EncryptedWithIdentity(inner, _) => self.get_host_from(&inner),
        };
    }

    #[inline]
    pub(crate) fn get_host_from(&self, endpoint_url: &url::Url) -> String {
        let mut host = match endpoint_url.host_str() {
            Some(h) => String::from(h),
            None => String::from("localhost"),
        };

        if let Some(port) = endpoint_url.port_or_known_default() {
            host = format!("{}:{}", host, port);
        }

        host
    }

    #[inline]
    pub fn scheme(&self) -> &str {
        return match self {
            Self::Plain(inner) => inner.scheme(),
            Self::Encrypted(inner) => inner.scheme(),
            Self::EncryptedWithIdentity(inner, _) => inner.scheme(),
        };
    }

    #[inline]
    pub fn query(&self) -> Option<String> {
        return match self {
            Self::Plain(inner) => self.get_query_params(&inner),
            Self::Encrypted(inner) => self.get_query_params(&inner),
            Self::EncryptedWithIdentity(inner, _) => self.get_query_params(&inner),
        };
    }

    #[inline]
    pub(crate) fn get_query_params(&self, endpoint_url: &url::Url) -> Option<String> {
        match endpoint_url.query() {
            Some(query) => Some(String::from(query)),
            None => None,
        }
    }

    #[inline]
    pub fn path_and_query(&self) -> String {
        return match self {
            Self::Plain(inner) => self.get_path_with_query_params(&inner),
            Self::Encrypted(inner) => self.get_path_with_query_params(&inner),
            Self::EncryptedWithIdentity(inner, _) => self.get_path_with_query_params(&inner),
        };
    }

    #[inline]
    pub fn path(&self) -> String {
        return match self {
            Self::Plain(inner) => String::from(inner.path()),
            Self::Encrypted(inner) => String::from(inner.path()),
            Self::EncryptedWithIdentity(inner, _) => String::from(inner.path()),
        };
    }

    #[inline]
    pub(crate) fn get_path_with_query_params(&self, endpoint_url: &url::Url) -> String {
        match endpoint_url.query() {
            Some(query) => format!("{}?{}", endpoint_url.path(), query),
            None => endpoint_url.path().to_owned(),
        }
    }
}
