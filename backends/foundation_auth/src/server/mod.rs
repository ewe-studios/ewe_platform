pub mod config;
pub mod handlers;
pub mod idp_server;
pub mod models;
pub mod services;
pub mod storage;

pub use config::{IdpConfig, PasswordPolicy};
pub use idp_server::IdpServer;
pub use models::{AuthorizationCode, DeviceCode, OAuthClient, RefreshToken, User};
pub use services::{
    TokenPair, TokenService, TokenServiceError,
    SessionService, SessionServiceError,
    hash_password, validate_password, verify_password, UserServiceError,
    create_client_record, ClientServiceError,
};
pub use handlers::{
    OidcDiscoveryDocument, DeviceAuthResponse, IdpError, IdpHandlerCore, ServeAdapter,
    HandlerResponse,
};
pub use storage::{HandlerStorage, StorageOpError, find_client_by_id, find_user_by_email};
