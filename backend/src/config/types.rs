use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    /// Path to an env file relative to the project directory
    #[serde(default, rename = "env-file")]
    pub env_file: Option<String>,
    #[serde(default)]
    pub services: HashMap<String, ServiceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ServiceConfig {
    Docker(DockerServiceConfig),
    Process(ProcessServiceConfig),
    /// Docker compose service (externally managed, read-only)
    #[serde(rename = "compose")]
    Compose(ComposeServiceConfig),
}

/// Configuration for a docker compose managed service (externally managed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceConfig {
    /// The container ID
    pub container_id: String,
    /// The image being used
    pub image: String,
    /// Host port the service is exposed on (if any)
    pub host_port: Option<u16>,
    /// Internal port the service listens on
    pub internal_port: Option<u16>,
    /// The compose project name
    pub compose_project: String,
    /// Volume mounts (extracted from container)
    #[serde(default)]
    pub volumes: Vec<String>,
    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerServiceConfig {
    pub image: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default)]
    pub ports: Vec<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Internal port that the service listens on (e.g., 5432 for postgres)
    #[serde(default)]
    pub internal_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessServiceConfig {
    pub command: String,
    #[serde(default = "default_working_dir")]
    pub working_dir: String,
    /// Path to an env file relative to the project directory
    #[serde(default, rename = "env-file")]
    pub env_file: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

fn default_working_dir() -> String {
    ".".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub config: Option<ProjectConfig>,
    pub has_docker_compose: bool,
}

impl Project {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            config: None,
            has_docker_compose: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceStatus::Stopped => write!(f, "Stopped"),
            ServiceStatus::Starting => write!(f, "Starting"),
            ServiceStatus::Running => write!(f, "Running"),
            ServiceStatus::Stopping => write!(f, "Stopping"),
            ServiceStatus::Error => write!(f, "Error"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub name: String,
    pub status: ServiceStatus,
    pub port: Option<u16>,
    pub container_id: Option<String>,
    pub process_id: Option<u32>,
    pub error_message: Option<String>,
}

impl ServiceState {
    pub fn new(name: String) -> Self {
        Self {
            name,
            status: ServiceStatus::Stopped,
            port: None,
            container_id: None,
            process_id: None,
            error_message: None,
        }
    }
}

/// Application-wide settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub scan_directories: Vec<PathBuf>,
    pub port_ranges: PortRanges,
}

impl Default for AppSettings {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            scan_directories: vec![
                home.join("developer"),
                home.join("projects"),
                home.join("Developer"),
                home.join("Projects"),
            ],
            port_ranges: PortRanges::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortRanges {
    pub postgres: (u16, u16),
    pub redis: (u16, u16),
    pub mysql: (u16, u16),
    pub mongo: (u16, u16),
    pub http: (u16, u16),
    pub generic: (u16, u16),
}

impl Default for PortRanges {
    fn default() -> Self {
        Self {
            postgres: (5432, 5500),
            redis: (6379, 6450),
            mysql: (3306, 3400),
            mongo: (27017, 27100),
            http: (3000, 3100),
            generic: (8000, 9000),
        }
    }
}
