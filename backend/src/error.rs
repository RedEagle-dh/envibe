use thiserror::Error;

#[derive(Error, Debug)]
pub enum EnvibeError {
    #[error("Docker error: {0}")]
    Docker(#[from] bollard::errors::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Port allocation failed: {0}")]
    PortAllocation(String),

    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Project not found: {0}")]
    ProjectNotFound(String),

    #[error("Process error: {0}")]
    Process(String),

    #[error("Docker daemon not running")]
    DockerNotRunning,

    #[error("Container not found: {0}")]
    ContainerNotFound(String),

    #[error("Variable interpolation error: {0}")]
    Interpolation(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

// Alias for convenience
pub use EnvibeError as Error;

pub type Result<T> = std::result::Result<T, EnvibeError>;
