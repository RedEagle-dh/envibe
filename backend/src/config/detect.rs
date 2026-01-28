use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

use crate::config::types::{ComposeServiceConfig, DockerServiceConfig, ProcessServiceConfig, ProjectConfig, ServiceConfig};
use crate::docker::ComposeContainer;
use crate::error::Result;

/// Detect services from docker-compose.yml
pub async fn detect_from_docker_compose(project_path: &Path) -> Result<Option<ProjectConfig>> {
    let compose_path = project_path.join("docker-compose.yml");
    if !compose_path.exists() {
        let compose_yaml_path = project_path.join("docker-compose.yaml");
        if !compose_yaml_path.exists() {
            return Ok(None);
        }
        return parse_docker_compose(&compose_yaml_path, project_path).await;
    }
    parse_docker_compose(&compose_path, project_path).await
}

async fn parse_docker_compose(path: &Path, project_path: &Path) -> Result<Option<ProjectConfig>> {
    let content = fs::read_to_string(path).await?;
    let compose: serde_yaml::Value = serde_yaml::from_str(&content)?;

    let project_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut services = HashMap::new();

    if let Some(compose_services) = compose.get("services").and_then(|s| s.as_mapping()) {
        for (name, config) in compose_services {
            let service_name = name.as_str().unwrap_or("unknown").to_string();

            if let Some(image) = config.get("image").and_then(|i| i.as_str()) {
                let env = extract_env(config);
                let volumes = extract_volumes(config);
                let ports = extract_ports(config);
                let depends_on = extract_depends_on(config);
                let internal_port = detect_internal_port(image);

                let docker_config = DockerServiceConfig {
                    image: image.to_string(),
                    env,
                    volumes,
                    ports,
                    command: config.get("command").and_then(|c| c.as_str()).map(String::from),
                    depends_on,
                    internal_port,
                };

                services.insert(service_name, ServiceConfig::Docker(docker_config));
            }
        }
    }

    if services.is_empty() {
        return Ok(None);
    }

    Ok(Some(ProjectConfig {
        name: project_name,
        env_file: None,
        services,
        agents: HashMap::new(),
    }))
}

fn extract_env(config: &serde_yaml::Value) -> HashMap<String, String> {
    let mut env = HashMap::new();

    if let Some(environment) = config.get("environment") {
        match environment {
            serde_yaml::Value::Mapping(map) => {
                for (key, value) in map {
                    if let (Some(k), Some(v)) = (key.as_str(), value.as_str()) {
                        env.insert(k.to_string(), v.to_string());
                    }
                }
            }
            serde_yaml::Value::Sequence(seq) => {
                for item in seq {
                    if let Some(s) = item.as_str() {
                        if let Some((k, v)) = s.split_once('=') {
                            env.insert(k.to_string(), v.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    env
}

fn extract_volumes(config: &serde_yaml::Value) -> Vec<String> {
    config
        .get("volumes")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_ports(config: &serde_yaml::Value) -> Vec<String> {
    config
        .get("ports")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_depends_on(config: &serde_yaml::Value) -> Vec<String> {
    config
        .get("depends_on")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Detect the internal port based on the image name
fn detect_internal_port(image: &str) -> Option<u16> {
    let image_lower = image.to_lowercase();

    if image_lower.contains("postgres") {
        Some(5432)
    } else if image_lower.contains("redis") {
        Some(6379)
    } else if image_lower.contains("mysql") || image_lower.contains("mariadb") {
        Some(3306)
    } else if image_lower.contains("mongo") {
        Some(27017)
    } else if image_lower.contains("nginx") {
        Some(80)
    } else if image_lower.contains("httpd") || image_lower.contains("apache") {
        Some(80)
    } else {
        None
    }
}

/// Detect common dev commands in the project
pub async fn detect_dev_command(project_path: &Path) -> Option<ProcessServiceConfig> {
    // Check for package.json
    let package_json = project_path.join("package.json");
    if package_json.exists() {
        if let Ok(content) = fs::read_to_string(&package_json).await {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) {
                    // Check for common dev scripts
                    for script_name in ["dev", "start", "serve"] {
                        if scripts.contains_key(script_name) {
                            // Detect package manager
                            let cmd = if project_path.join("bun.lockb").exists() {
                                format!("bun run {}", script_name)
                            } else if project_path.join("pnpm-lock.yaml").exists() {
                                format!("pnpm run {}", script_name)
                            } else if project_path.join("yarn.lock").exists() {
                                format!("yarn {}", script_name)
                            } else {
                                format!("npm run {}", script_name)
                            };

                            return Some(ProcessServiceConfig {
                                command: cmd,
                                working_dir: ".".to_string(),
                                env_file: None,
                                env: HashMap::new(),
                                depends_on: vec![],
                            });
                        }
                    }
                }
            }
        }
    }

    // Check for Cargo.toml (Rust)
    if project_path.join("Cargo.toml").exists() {
        return Some(ProcessServiceConfig {
            command: "cargo run".to_string(),
            working_dir: ".".to_string(),
            env_file: None,
            env: HashMap::new(),
            depends_on: vec![],
        });
    }

    // Check for go.mod (Go)
    if project_path.join("go.mod").exists() {
        return Some(ProcessServiceConfig {
            command: "go run .".to_string(),
            working_dir: ".".to_string(),
            env_file: None,
            env: HashMap::new(),
            depends_on: vec![],
        });
    }

    // Check for pyproject.toml or setup.py (Python)
    if project_path.join("pyproject.toml").exists() || project_path.join("setup.py").exists() {
        return Some(ProcessServiceConfig {
            command: "python -m main".to_string(),
            working_dir: ".".to_string(),
            env_file: None,
            env: HashMap::new(),
            depends_on: vec![],
        });
    }

    None
}

pub fn docker_compose_exists(project_path: &Path) -> bool {
    project_path.join("docker-compose.yml").exists()
        || project_path.join("docker-compose.yaml").exists()
}

/// Create service configs from running docker compose containers
/// This allows automatic discovery of services without needing a config file
pub fn services_from_compose_containers(
    containers: &[ComposeContainer],
    project_name: &str,
) -> HashMap<String, ServiceConfig> {
    let mut services = HashMap::new();

    // Filter containers for this project (docker compose uses directory name as project)
    let project_lower = project_name.to_lowercase().replace(['-', '_', ' '], "");

    for container in containers {
        let compose_project_lower = container.project.to_lowercase().replace(['-', '_', ' '], "");

        // Check if container belongs to this project
        if compose_project_lower == project_lower || container.project == project_name {
            let host_port = container.ports.first().map(|(_, host)| *host);
            let internal_port = container.ports.first().map(|(internal, _)| *internal)
                .or_else(|| detect_internal_port(&container.image));

            let compose_config = ComposeServiceConfig {
                container_id: container.id.clone(),
                image: container.image.clone(),
                host_port,
                internal_port,
                compose_project: container.project.clone(),
                volumes: container.volumes.clone(),
                env: HashMap::new(), // Env vars would require inspecting container
            };

            services.insert(container.service.clone(), ServiceConfig::Compose(compose_config));
        }
    }

    services
}

/// Merge compose services into an existing project config
pub fn merge_compose_services(
    config: &mut ProjectConfig,
    compose_services: HashMap<String, ServiceConfig>,
) {
    for (name, service) in compose_services {
        // Don't overwrite existing services (user config takes precedence)
        if !config.services.contains_key(&name) {
            config.services.insert(name, service);
        }
    }
}
