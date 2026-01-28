use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, ListContainersOptions, LogOutput, LogsOptions,
    RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::models::{ContainerStateStatusEnum, HostConfig, PortBinding};
use futures::StreamExt;
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::config::{DockerServiceConfig, ServiceStatus};
use crate::error::{EnvibeError, Result};

pub struct DockerClient {
    docker: Docker,
}

impl DockerClient {
    pub async fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .map_err(|_| EnvibeError::DockerNotRunning)?;

        // Verify connection
        docker
            .ping()
            .await
            .map_err(|_| EnvibeError::DockerNotRunning)?;

        Ok(Self { docker })
    }

    /// Create and start a container for a service
    pub async fn start_container(
        &self,
        project: &str,
        service_name: &str,
        config: &DockerServiceConfig,
        host_port: u16,
    ) -> Result<String> {
        let container_name = format!("{}_{}", project, service_name);

        // Determine internal port early so we can check existing container's binding
        let internal_port = config.internal_port.unwrap_or_else(|| {
            self.detect_internal_port(&config.image).unwrap_or(8080)
        });

        tracing::info!(
            "start_container: {} with requested host_port={}, internal_port={}",
            container_name, host_port, internal_port
        );

        // Check if container already exists
        if let Some(id) = self.get_container_id(&container_name).await? {
            // Container exists, check its state and port binding
            let info = self.docker.inspect_container(&id, None).await?;

            // Get existing port binding for logging
            let existing_port = info
                .host_config
                .as_ref()
                .and_then(|hc| hc.port_bindings.as_ref())
                .and_then(|pb| pb.get(&format!("{}/tcp", internal_port)))
                .and_then(|bindings| bindings.as_ref())
                .and_then(|bindings| bindings.first())
                .and_then(|binding| binding.host_port.as_ref())
                .cloned();

            tracing::info!(
                "Existing container {} has port binding: {:?}, requested: {}",
                container_name, existing_port, host_port
            );

            // Check if the port binding matches what we want
            let port_matches = existing_port
                .as_ref()
                .map(|p| p == &host_port.to_string())
                .unwrap_or(false);

            if port_matches {
                // Port matches, just start if not running
                if let Some(state) = &info.state {
                    if state.running.unwrap_or(false) {
                        return Ok(id);
                    }
                }
                // Start existing container
                self.docker
                    .start_container(&id, None::<StartContainerOptions<String>>)
                    .await?;
                return Ok(id);
            } else {
                // Port changed, need to remove and recreate container
                tracing::info!(
                    "Port changed for {}, removing container to recreate with new port {}",
                    container_name,
                    host_port
                );
                // Stop if running
                if info.state.as_ref().and_then(|s| s.running).unwrap_or(false) {
                    let _ = self.stop_container(&id).await;
                }
                // Remove container
                self.remove_container(&id).await?;
            }
        }

        // Pull image if needed
        self.pull_image(&config.image).await?;

        // Build environment variables
        let env: Vec<String> = config
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        // Build port bindings
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            format!("{}/tcp", internal_port),
            Some(vec![PortBinding {
                host_ip: Some("127.0.0.1".to_string()),
                host_port: Some(host_port.to_string()),
            }]),
        );

        // Build exposed ports
        let mut exposed_ports = HashMap::new();
        exposed_ports.insert(format!("{}/tcp", internal_port), HashMap::new());

        // Build volume bindings, prefix named volumes with project name (docker-compose convention)
        let binds: Vec<String> = config
            .volumes
            .iter()
            .map(|v| {
                if let Some((source, dest)) = v.split_once(':') {
                    // Named volume if source doesn't start with ., /, or ~
                    if !source.starts_with('.') && !source.starts_with('/') && !source.starts_with('~') {
                        // Check if already prefixed with project name
                        let prefixed = format!("{}_{}", project, source);
                        if source.starts_with(&format!("{}_", project)) {
                            v.clone()
                        } else {
                            format!("{}:{}", prefixed, dest)
                        }
                    } else {
                        v.clone()
                    }
                } else {
                    v.clone()
                }
            })
            .collect();

        // Create host config
        let host_config = HostConfig {
            port_bindings: Some(port_bindings),
            binds: if binds.is_empty() { None } else { Some(binds) },
            auto_remove: Some(false),
            ..Default::default()
        };

        // Create container config
        let container_config = Config {
            image: Some(config.image.clone()),
            env: Some(env),
            exposed_ports: Some(exposed_ports),
            host_config: Some(host_config),
            cmd: config.command.as_ref().map(|c| vec![c.clone()]),
            labels: Some(HashMap::from([
                ("envibe.project".to_string(), project.to_string()),
                ("envibe.service".to_string(), service_name.to_string()),
            ])),
            ..Default::default()
        };

        // Create container
        let create_options = CreateContainerOptions {
            name: container_name.clone(),
            platform: None,
        };

        let response = self
            .docker
            .create_container(Some(create_options), container_config)
            .await?;

        // Start container
        self.docker
            .start_container(&response.id, None::<StartContainerOptions<String>>)
            .await?;

        Ok(response.id)
    }

    /// Stop a container
    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        let options = StopContainerOptions { t: 10 };
        self.docker.stop_container(container_id, Some(options)).await?;
        Ok(())
    }

    /// Start/restart an existing container
    pub async fn restart_container(&self, container_id: &str) -> Result<()> {
        self.docker
            .start_container(container_id, None::<StartContainerOptions<String>>)
            .await?;
        Ok(())
    }

    /// Remove a container
    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
        let options = RemoveContainerOptions {
            force: true,
            ..Default::default()
        };
        self.docker
            .remove_container(container_id, Some(options))
            .await?;
        Ok(())
    }

    /// Get container status
    pub async fn get_container_status(&self, container_id: &str) -> Result<ServiceStatus> {
        let info = self.docker.inspect_container(container_id, None).await?;

        let status = match info.state.and_then(|s| s.status) {
            Some(ContainerStateStatusEnum::RUNNING) => ServiceStatus::Running,
            Some(ContainerStateStatusEnum::CREATED) => ServiceStatus::Starting,
            Some(ContainerStateStatusEnum::RESTARTING) => ServiceStatus::Starting,
            Some(ContainerStateStatusEnum::REMOVING) => ServiceStatus::Stopping,
            Some(ContainerStateStatusEnum::PAUSED) => ServiceStatus::Stopped,
            Some(ContainerStateStatusEnum::EXITED) => ServiceStatus::Stopped,
            Some(ContainerStateStatusEnum::DEAD) => ServiceStatus::Error,
            _ => ServiceStatus::Stopped,
        };

        Ok(status)
    }

    /// Stream container logs, prefixing each line with the service name
    pub fn stream_logs(&self, container_id: &str, service_name: &str, tx: mpsc::Sender<String>) {
        let docker = self.docker.clone();
        let id = container_id.to_string();
        let prefix = service_name.to_string();

        tokio::spawn(async move {
            let options = LogsOptions::<String> {
                stdout: true,
                stderr: true,
                follow: true,
                tail: "100".to_string(),
                ..Default::default()
            };

            let mut stream = docker.logs(&id, Some(options));

            while let Some(result) = stream.next().await {
                match result {
                    Ok(output) => {
                        let raw = match output {
                            LogOutput::StdOut { message } => {
                                String::from_utf8_lossy(&message).to_string()
                            }
                            LogOutput::StdErr { message } => {
                                String::from_utf8_lossy(&message).to_string()
                            }
                            _ => continue,
                        };
                        // Docker output may contain multiple lines per chunk
                        for line in raw.lines() {
                            if !line.is_empty() {
                                if tx.send(format!("[{}] {}", prefix, line)).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    /// Pull an image if not present
    async fn pull_image(&self, image: &str) -> Result<()> {
        // Check if image exists locally
        if self.docker.inspect_image(image).await.is_ok() {
            return Ok(());
        }

        let options = CreateImageOptions {
            from_image: image,
            ..Default::default()
        };

        let mut stream = self.docker.create_image(Some(options), None, None);

        while let Some(result) = stream.next().await {
            if let Err(e) = result {
                return Err(EnvibeError::Docker(e));
            }
        }

        Ok(())
    }

    /// Get container ID by name
    async fn get_container_id(&self, name: &str) -> Result<Option<String>> {
        let mut filters = HashMap::new();
        filters.insert("name", vec![name]);

        let options = ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        };

        let containers = self.docker.list_containers(Some(options)).await?;

        Ok(containers.first().and_then(|c| c.id.clone()))
    }

    /// Detect internal port from image name
    fn detect_internal_port(&self, image: &str) -> Option<u16> {
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
        } else {
            None
        }
    }

    /// List envibe containers
    /// If `running_only` is true, only returns running containers
    /// If false, returns all containers (including stopped)
    pub async fn list_envibe_containers_filtered(&self, running_only: bool) -> Result<Vec<(String, String, String)>> {
        let mut filters = HashMap::new();
        filters.insert("label", vec!["envibe.project"]);

        let options = ListContainersOptions {
            all: !running_only,  // all=false means only running, all=true means all
            filters,
            ..Default::default()
        };

        let containers = self.docker.list_containers(Some(options)).await?;

        let result: Vec<(String, String, String)> = containers
            .into_iter()
            .filter_map(|c| {
                let id = c.id?;
                let labels = c.labels?;
                let project = labels.get("envibe.project")?.clone();
                let service = labels.get("envibe.service")?.clone();
                Some((id, project, service))
            })
            .collect();

        Ok(result)
    }

    /// List all envibe containers (including stopped) - for finding containers to start
    pub async fn list_envibe_containers(&self) -> Result<Vec<(String, String, String)>> {
        self.list_envibe_containers_filtered(false).await
    }

    /// List only running envibe containers - for status detection
    pub async fn list_running_envibe_containers(&self) -> Result<Vec<(String, String, String)>> {
        self.list_envibe_containers_filtered(true).await
    }

    /// List all running docker compose containers with their project and service info
    pub async fn list_compose_containers(&self) -> Result<Vec<ComposeContainer>> {
        let mut filters = HashMap::new();
        filters.insert("label", vec!["com.docker.compose.project"]);

        let options = ListContainersOptions {
            all: false, // Only running containers
            filters,
            ..Default::default()
        };

        let containers = self.docker.list_containers(Some(options)).await?;

        let result: Vec<ComposeContainer> = containers
            .into_iter()
            .filter_map(|c| {
                let id = c.id?;
                let labels = c.labels?;
                let project = labels.get("com.docker.compose.project")?.clone();
                let service = labels.get("com.docker.compose.service")?.clone();
                let image = c.image?;

                // Extract port mappings
                let ports: Vec<(u16, u16)> = c.ports.unwrap_or_default()
                    .into_iter()
                    .filter_map(|p| {
                        let private = p.private_port;
                        let public = p.public_port?;
                        Some((private, public))
                    })
                    .collect();

                // Extract volume mounts
                let volumes: Vec<String> = c.mounts.unwrap_or_default()
                    .into_iter()
                    .filter_map(|m| {
                        let destination = m.destination?;
                        // For named volumes, use the volume name; for bind mounts, use the source path
                        let is_volume = matches!(m.typ, Some(bollard::models::MountPointTypeEnum::VOLUME));
                        let source = if is_volume {
                            m.name.unwrap_or_else(|| m.source.unwrap_or_default())
                        } else {
                            m.source?
                        };
                        Some(format!("{}:{}", source, destination))
                    })
                    .collect();

                Some(ComposeContainer {
                    id,
                    project,
                    service,
                    image,
                    ports,
                    volumes,
                })
            })
            .collect();

        Ok(result)
    }
}

/// Represents a running docker compose container
#[derive(Debug, Clone)]
pub struct ComposeContainer {
    pub id: String,
    pub project: String,
    pub service: String,
    pub image: String,
    pub ports: Vec<(u16, u16)>, // (container_port, host_port)
    pub volumes: Vec<String>,   // source:destination format
}
