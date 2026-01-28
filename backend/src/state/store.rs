use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::config::ServiceState;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    /// Maps project name to service states
    pub services: HashMap<String, HashMap<String, ServiceState>>,
    /// Known project paths
    pub known_projects: Vec<PathBuf>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
            known_projects: Vec::new(),
        }
    }

    /// Load state from disk
    pub async fn load(data_dir: &PathBuf) -> Result<Self> {
        let path = data_dir.join("state.json");
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(&path).await?;
        let state: AppState = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// Save state to disk
    pub async fn save(&self, data_dir: &PathBuf) -> Result<()> {
        let path = data_dir.join("state.json");
        let content = serde_json::to_string_pretty(&self)?;
        fs::write(&path, content).await?;
        Ok(())
    }

    /// Get service state for a project
    pub fn get_service_state(&self, project: &str, service: &str) -> Option<&ServiceState> {
        self.services
            .get(project)
            .and_then(|services| services.get(service))
    }

    /// Get mutable service state for a project
    pub fn get_service_state_mut(&mut self, project: &str, service: &str) -> Option<&mut ServiceState> {
        self.services
            .get_mut(project)
            .and_then(|services| services.get_mut(service))
    }

    /// Set service state for a project
    pub fn set_service_state(&mut self, project: &str, state: ServiceState) {
        let project_services = self.services.entry(project.to_string()).or_default();
        project_services.insert(state.name.clone(), state);
    }

    /// Get all service states for a project
    pub fn get_project_services(&self, project: &str) -> Option<&HashMap<String, ServiceState>> {
        self.services.get(project)
    }

    /// Initialize service states for a project from config
    pub fn init_project_services(&mut self, project: &str, service_names: Vec<String>) {
        let project_services = self.services.entry(project.to_string()).or_default();
        for name in service_names {
            if !project_services.contains_key(&name) {
                project_services.insert(name.clone(), ServiceState::new(name));
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectRegistry {
    pub paths: Vec<PathBuf>,
}

impl ProjectRegistry {
    pub async fn load(data_dir: &PathBuf) -> Result<Self> {
        let path = data_dir.join("projects.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path).await?;
        let registry: ProjectRegistry = serde_json::from_str(&content)?;
        Ok(registry)
    }

    pub async fn save(&self, data_dir: &PathBuf) -> Result<()> {
        let path = data_dir.join("projects.json");
        let content = serde_json::to_string_pretty(&self)?;
        fs::write(&path, content).await?;
        Ok(())
    }

    pub fn add(&mut self, path: PathBuf) -> bool {
        if !self.paths.contains(&path) {
            self.paths.push(path);
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, path: &PathBuf) -> bool {
        let len = self.paths.len();
        self.paths.retain(|p| p != path);
        self.paths.len() != len
    }
}

/// Ensure the data directory exists
pub async fn ensure_data_dir() -> Result<PathBuf> {
    let data_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".envibe");

    if !data_dir.exists() {
        fs::create_dir_all(&data_dir).await?;
    }

    // Create logs subdirectory
    let logs_dir = data_dir.join("logs");
    if !logs_dir.exists() {
        fs::create_dir_all(&logs_dir).await?;
    }

    Ok(data_dir)
}
