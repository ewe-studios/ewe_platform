use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevServer {
    pub addr: String,
    pub port: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppServer {
    pub addr: String,
    pub port: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub dev: Option<DevServer>,
    pub app: AppServer,
}
