//! Constants for Hugging Face Hub API client.

/// Default Hugging Face Hub API endpoint.
pub const HF_DEFAULT_ENDPOINT: &str = "https://huggingface.co/api";

/// Environment variable names for configuration.
pub const HF_ENDPOINT_ENV: &str = "HF_ENDPOINT";
pub const HF_TOKEN_ENV: &str = "HF_TOKEN";
pub const HF_TOKEN_PATH_ENV: &str = "HF_TOKEN_PATH";
pub const HF_HOME_ENV: &str = "HF_HOME";
pub const HF_HUB_DISABLE_IMPLICIT_TOKEN_ENV: &str = "HF_HUB_DISABLE_IMPLICIT_TOKEN";

/// Default cache directory name.
pub const HF_DEFAULT_CACHE_DIR: &str = ".cache/huggingface";

/// Default token filename.
pub const HF_TOKEN_FILENAME: &str = "token";

/// User agent for API requests.
pub const HF_USER_AGENT: &str = "huggingface-hub-rust/0.1.0";

/// Default revision for file operations.
pub const HF_DEFAULT_REVISION: &str = "main";

/// Repository type prefixes for URLs.
pub const HF_REPO_PREFIX_DATASET: &str = "datasets/";
pub const HF_REPO_PREFIX_SPACE: &str = "spaces/";
pub const HF_REPO_PREFIX_KERNEL: &str = "kernels/";

/// API endpoint paths.
pub const HF_API_WHOAMI: &str = "/api/whoami-v2";
pub const HF_API_MODELS: &str = "/api/models";
pub const HF_API_DATASETS: &str = "/api/datasets";
pub const HF_API_SPACES: &str = "/api/spaces";
pub const HF_API_REPOS_CREATE: &str = "/api/repos/create";
pub const HF_API_REPOS_DELETE: &str = "/api/repos/delete";
pub const HF_API_REPOS_MOVE: &str = "/api/repos/move";
