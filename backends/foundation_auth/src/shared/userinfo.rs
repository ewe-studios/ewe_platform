//! UserInfo Client — fetches user profile from OIDC `/userinfo` endpoint.
//!
//! Sends a GET request with a Bearer token and parses the response into
//! standard OIDC UserInfo claims.

use serde::{Deserialize, Serialize};

/// User profile from OIDC UserInfo endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// Subject identifier (required).
    pub sub: String,
    /// Full name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Given name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub given_name: Option<String>,
    /// Family name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,
    /// Middle name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middle_name: Option<String>,
    /// Nickname.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    /// Preferred username.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
    /// Profile URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Picture URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// Website URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    /// Email address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Whether the email has been verified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
    /// Gender.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    /// Birthdate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthdate: Option<String>,
    /// Timezone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoneinfo: Option<String>,
    /// Locale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Phone number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,
    /// Whether the phone number has been verified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number_verified: Option<bool>,
    /// Address (structured or string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<serde_json::Value>,
    /// Last updated timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// Group memberships (non-standard, common extension).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<String>>,
}

impl UserInfo {
    /// Validate that required fields are present.
    ///
    /// # Errors
    ///
    /// Returns `UserInfoError::MissingSubject` if `sub` is empty.
    pub fn validate(&self) -> Result<(), UserInfoError> {
        if self.sub.is_empty() {
            return Err(UserInfoError::MissingSubject);
        }
        Ok(())
    }
}

/// UserInfo client — fetches user profile from OIDC provider.
pub struct UserInfoClient;

impl UserInfoClient {
    /// Fetch user profile using the given access token.
    ///
    /// # Errors
    ///
    /// Returns `UserInfoError` if the request fails, the response is not 200,
    /// or the JSON cannot be parsed.
    pub async fn fetch(
        userinfo_url: &str,
        access_token: &str,
    ) -> Result<UserInfo, UserInfoError> {
        let body = fetch_userinfo(userinfo_url, access_token).await?;
        let info: UserInfo = serde_json::from_str(&body)
            .map_err(|e| UserInfoError::ParseError(e.to_string()))?;
        info.validate()?;
        Ok(info)
    }
}

/// UserInfo-related errors.
#[derive(Debug)]
pub enum UserInfoError {
    /// URL is invalid.
    InvalidUrl(String),
    /// HTTP fetch failed.
    FetchFailed(String),
    /// 401 Unauthorized.
    Unauthorized(String),
    /// JSON parse failed.
    ParseError(String),
    /// `sub` field is missing.
    MissingSubject,
}

impl core::fmt::Display for UserInfoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            UserInfoError::InvalidUrl(s) => write!(f, "UserInfo invalid URL: {s}"),
            UserInfoError::FetchFailed(s) => write!(f, "UserInfo fetch failed: {s}"),
            UserInfoError::Unauthorized(s) => write!(f, "UserInfo unauthorized: {s}"),
            UserInfoError::ParseError(s) => write!(f, "UserInfo parse error: {s}"),
            UserInfoError::MissingSubject => write!(f, "UserInfo missing subject"),
        }
    }
}

impl std::error::Error for UserInfoError {}

// ===========================================================================
// HTTP fetch — uses the cross-platform HttpClient trait (native + wasm).
// ===========================================================================

async fn fetch_userinfo(
    url: &str,
    access_token: &str,
) -> Result<String, UserInfoError> {
    use foundation_core::url::Uri;
    use foundation_netio::http::default_http_client;
    use foundation_netio::shared::client::request::PreparedRequest;
    use foundation_netio::shared::http::{
        SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod,
    };

    let client = default_http_client();
    let uri = Uri::parse(url)
        .map_err(|e| UserInfoError::FetchFailed(format!("invalid URL: {e}")))?;

    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::ACCEPT, vec!["application/json".into()]);
    headers.insert(
        SimpleHeader::AUTHORIZATION,
        vec![format!("Bearer {access_token}")],
    );

    let req = PreparedRequest {
        method: SimpleMethod::GET,
        url: uri,
        headers,
        body: SendSafeBody::None,
        extensions: Default::default(),
    };

    let resp = client
        .send_async(req)
        .await
        .map_err(|e| UserInfoError::FetchFailed(e.to_string()))?;

    let status: usize = resp.get_status().into();

    if status == 401 {
        let body = match resp.get_body_ref() {
            SendSafeBody::Text(t) => t.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            _ => String::new(),
        };
        return Err(UserInfoError::Unauthorized(body));
    }

    if !(200..300).contains(&status) {
        let body = match resp.get_body_ref() {
            SendSafeBody::Text(t) => t.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            _ => String::new(),
        };
        return Err(UserInfoError::FetchFailed(format!(
            "HTTP {status}: {body}"
        )));
    }

    match resp.get_body_ref() {
        SendSafeBody::Text(t) => Ok(t.clone()),
        SendSafeBody::Bytes(b) => String::from_utf8(b.clone())
            .map_err(|e| UserInfoError::FetchFailed(e.to_string())),
        _ => Ok(String::new()),
    }
}
