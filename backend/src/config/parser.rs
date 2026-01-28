use std::path::Path;
use tokio::fs;

use crate::config::types::{ProjectConfig, ServiceConfig};
use crate::error::{EnvibeError, Result};

pub async fn parse_config(path: &Path) -> Result<ProjectConfig> {
    let content = fs::read_to_string(path)
        .await
        .map_err(|e| EnvibeError::Config(format!("Failed to read config file: {}", e)))?;

    let mut config: ProjectConfig = serde_yaml::from_str(&content)?;

    // Merge agents into services as ServiceConfig::Agent
    for (name, agent_config) in std::mem::take(&mut config.agents) {
        if !config.services.contains_key(&name) {
            config.services.insert(name, ServiceConfig::Agent(agent_config));
        }
    }

    Ok(config)
}

pub async fn config_exists(project_path: &Path) -> bool {
    project_path.join(".envibe.yaml").exists() || project_path.join(".envibe.yml").exists()
}

pub async fn get_config_path(project_path: &Path) -> Option<std::path::PathBuf> {
    let yaml_path = project_path.join(".envibe.yaml");
    if yaml_path.exists() {
        return Some(yaml_path);
    }

    let yml_path = project_path.join(".envibe.yml");
    if yml_path.exists() {
        return Some(yml_path);
    }

    None
}
