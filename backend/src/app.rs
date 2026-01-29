use std::collections::HashMap;
use std::path::PathBuf;

use ratatui::widgets::ListState;
use tokio::sync::mpsc;
use walkdir::WalkDir;

use crate::config::{
    detect_from_docker_compose, docker_compose_exists, get_config_path,
    merge_compose_services, parse_config, services_from_compose_containers,
    AppSettings, ComposeServiceConfig, DockerServiceConfig, ProcessServiceConfig,
    Project, ProjectConfig, ServiceConfig, ServiceState, ServiceStatus,
};
use crate::docker::DockerClient;
use crate::error::{EnvibeError, Result};
use crate::ports::{PortRegistry, ServiceType};
use crate::process::{interpolate_env_map, parse_env_file, ProcessManager};
use crate::state::AppState;
use crate::tui::Panel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    Error,
    Info,
    Normal,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub text: String,
    pub kind: LogKind,
}

impl LogEntry {
    pub fn new(text: String) -> Self {
        let kind = classify_log(&text);
        Self { text, kind }
    }
}

fn classify_log(text: &str) -> LogKind {
    if text.contains("ERR") || contains_error_ci(text) {
        LogKind::Error
    } else if text.starts_with('[') {
        LogKind::Info
    } else {
        LogKind::Normal
    }
}

fn contains_error_ci(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes
        .windows(5)
        .any(|window| window.eq_ignore_ascii_case(b"error"))
}

pub struct App {
    pub projects: Vec<Project>,
    pub projects_state: ListState,
    pub services_state: ListState,
    pub state: AppState,
    pub port_registry: PortRegistry,
    pub settings: AppSettings,
    pub focused_panel: Panel,
    pub logs: Vec<LogEntry>,
    pub log_scroll: usize,
    pub follow_logs: bool,
    pub show_help: bool,
    pub should_quit: bool,
    pub data_dir: PathBuf,
    docker: Option<DockerClient>,
    process_manager: ProcessManager,
    log_tx: mpsc::Sender<String>,
    log_rx: mpsc::Receiver<String>,
}

impl App {
    pub async fn new(data_dir: PathBuf) -> Result<Self> {
        let state = AppState::load(&data_dir).await.unwrap_or_default();
        let port_registry = PortRegistry::load(&data_dir).await.unwrap_or_default();
        let settings = AppSettings::default();

        // Try to connect to Docker
        let docker = DockerClient::new().await.ok();

        let (log_tx, log_rx) = mpsc::channel(1000);

        let mut app = Self {
            projects: Vec::new(),
            projects_state: ListState::default(),
            services_state: ListState::default(),
            state,
            port_registry,
            settings,
            focused_panel: Panel::Projects,
            logs: Vec::new(),
            log_scroll: 0,
            follow_logs: true,
            show_help: false,
            should_quit: false,
            data_dir,
            docker,
            process_manager: ProcessManager::new(),
            log_tx,
            log_rx,
        };

        // Scan for projects
        app.scan_projects().await?;

        Ok(app)
    }

    /// Scan configured directories for projects
    pub async fn scan_projects(&mut self) -> Result<()> {
        self.projects.clear();

        // Clone scan directories to avoid borrow issues
        let scan_directories: Vec<PathBuf> = self.settings.scan_directories.clone();
        let mut errors: Vec<String> = Vec::new();

        // Query running docker compose containers for auto-discovery
        let compose_containers = if let Some(ref docker) = self.docker {
            docker.list_compose_containers().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        for scan_dir in scan_directories {
            if !scan_dir.exists() {
                continue;
            }

            // Walk first level directories
            for entry in WalkDir::new(&scan_dir)
                .max_depth(1)
                .min_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                // Skip hidden directories
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with('.'))
                    .unwrap_or(false)
                {
                    continue;
                }

                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let mut project = Project::new(name, path.to_path_buf());

                // Check for .envibe.yaml
                if let Some(config_path) = get_config_path(path).await {
                    match parse_config(&config_path).await {
                        Ok(config) => {
                            // Initialize service states
                            let service_names: Vec<String> =
                                config.services.keys().cloned().collect();
                            self.state
                                .init_project_services(&project.name, service_names);
                            project.config = Some(config);
                        }
                        Err(e) => {
                            errors.push(format!(
                                "Error parsing config for {}: {}",
                                project.name, e
                            ));
                        }
                    }
                }

                // Check for docker-compose.yml
                project.has_docker_compose = docker_compose_exists(path);

                // If no config but has docker-compose, try to detect from file
                if project.config.is_none() && project.has_docker_compose {
                    if let Ok(Some(config)) = detect_from_docker_compose(path).await {
                        let service_names: Vec<String> = config.services.keys().cloned().collect();
                        self.state
                            .init_project_services(&project.name, service_names);
                        project.config = Some(config);
                    }
                }

                // Auto-discover running docker compose containers for this project
                // This allows services like db, redis to be known without a config file
                let compose_services =
                    services_from_compose_containers(&compose_containers, &project.name);

                if !compose_services.is_empty() {
                    if let Some(ref mut config) = project.config {
                        // Merge compose services into existing config
                        merge_compose_services(config, compose_services);
                    } else {
                        // Create a new config from compose services
                        project.config = Some(ProjectConfig {
                            name: project.name.clone(),
                            env_file: None,
                            services: compose_services,
                        });
                    }

                    // Initialize service states for compose services
                    if let Some(ref config) = project.config {
                        let service_names: Vec<String> = config.services.keys().cloned().collect();
                        self.state
                            .init_project_services(&project.name, service_names);

                        // Mark compose services as already running
                        for (svc_name, svc_config) in &config.services {
                            if let ServiceConfig::Compose(compose_config) = svc_config {
                                let mut state = ServiceState::new(svc_name.clone());
                                state.status = ServiceStatus::Running;
                                state.port = compose_config.host_port;
                                state.container_id = Some(compose_config.container_id.clone());
                                self.state.set_service_state(&project.name, state);
                            }
                        }
                    }
                }

                self.projects.push(project);
            }
        }

        // Log any errors that occurred
        for error in errors {
            self.log(error);
        }

        // Sort projects by name
        self.projects.sort_by(|a, b| a.name.cmp(&b.name));

        // Select first project if available
        if !self.projects.is_empty() && self.projects_state.selected().is_none() {
            self.projects_state.select(Some(0));
        }

        Ok(())
    }

    /// Get the currently selected project
    pub fn current_project(&self) -> Option<&Project> {
        self.projects_state
            .selected()
            .and_then(|i| self.projects.get(i))
    }

    /// Get mutable reference to current project
    pub fn current_project_mut(&mut self) -> Option<&mut Project> {
        self.projects_state
            .selected()
            .and_then(|i| self.projects.get_mut(i))
    }

    /// Get the currently selected service name
    pub fn current_service_name(&self) -> Option<String> {
        let project = self.current_project()?;
        let config = project.config.as_ref()?;
        let idx = self.services_state.selected()?;
        config.services.keys().nth(idx).cloned()
    }

    /// Navigate to next item in focused panel
    pub fn navigate_down(&mut self) {
        match self.focused_panel {
            Panel::Projects => {
                let len = self.projects.len();
                if len > 0 {
                    let i = self.projects_state.selected().unwrap_or(0);
                    self.projects_state.select(Some((i + 1) % len));
                }
            }
            Panel::Services => {
                if let Some(project) = self.current_project() {
                    if let Some(config) = &project.config {
                        let len = config.services.len();
                        if len > 0 {
                            let i = self.services_state.selected().unwrap_or(0);
                            self.services_state.select(Some((i + 1) % len));
                        }
                    }
                }
            }
            Panel::Console => {
                if !self.follow_logs {
                    self.log_scroll = self.log_scroll.saturating_add(1);
                }
            }
        }
    }

    /// Navigate to previous item in focused panel
    pub fn navigate_up(&mut self) {
        match self.focused_panel {
            Panel::Projects => {
                let len = self.projects.len();
                if len > 0 {
                    let i = self.projects_state.selected().unwrap_or(0);
                    self.projects_state
                        .select(Some(if i == 0 { len - 1 } else { i - 1 }));
                }
            }
            Panel::Services => {
                if let Some(project) = self.current_project() {
                    if let Some(config) = &project.config {
                        let len = config.services.len();
                        if len > 0 {
                            let i = self.services_state.selected().unwrap_or(0);
                            self.services_state
                                .select(Some(if i == 0 { len - 1 } else { i - 1 }));
                        }
                    }
                }
            }
            Panel::Console => {
                if !self.follow_logs {
                    self.log_scroll = self.log_scroll.saturating_sub(1);
                }
            }
        }
    }

    /// Select project (when in projects panel)
    pub fn select_project(&mut self) {
        if let Some(project) = self.current_project() {
            if project.config.is_some() {
                self.focused_panel = Panel::Services;
                self.services_state.select(Some(0));
            }
        }
    }

    /// Toggle the selected service
    pub async fn toggle_service(&mut self) -> Result<()> {
        // Extract all needed data first to avoid borrow issues
        let (project_name, project_path, project_config, service_name, service_config) = {
            let project = match self.current_project() {
                Some(p) => p,
                None => return Ok(()),
            };

            let config = match &project.config {
                Some(c) => c.clone(),
                None => return Ok(()),
            };

            let svc_name = match self.current_service_name() {
                Some(n) => n,
                None => return Ok(()),
            };

            let svc_config = match config.services.get(&svc_name) {
                Some(c) => c.clone(),
                None => return Ok(()),
            };

            (
                project.name.clone(),
                project.path.clone(),
                config,
                svc_name,
                svc_config,
            )
        };

        // Get current status
        let status = self
            .state
            .get_service_state(&project_name, &service_name)
            .map(|s| s.status)
            .unwrap_or(ServiceStatus::Stopped);

        match status {
            ServiceStatus::Running => {
                self.stop_service(&project_name, &service_name, &service_config)
                    .await?;
            }
            ServiceStatus::Stopped | ServiceStatus::Error => {
                self.start_service(
                    &project_name,
                    &service_name,
                    &service_config,
                    &project_path,
                    &project_config,
                )
                .await?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Start a service
    async fn start_service(
        &mut self,
        project: &str,
        service_name: &str,
        config: &ServiceConfig,
        project_path: &PathBuf,
        project_config: &ProjectConfig,
    ) -> Result<()> {
        self.log(format!("Starting {}...", service_name));

        // Update state to starting
        let mut state = ServiceState::new(service_name.to_string());
        state.status = ServiceStatus::Starting;
        self.state.set_service_state(project, state);

        match config {
            ServiceConfig::Docker(docker_config) => {
                self.start_docker_service(project, service_name, docker_config)
                    .await?;
            }
            ServiceConfig::Process(process_config) => {
                self.start_process_service(
                    project,
                    service_name,
                    process_config,
                    project_path,
                    project_config,
                )
                .await?;
            }
            ServiceConfig::Compose(compose_config) => {
                // Compose services are externally managed - just stream logs
                self.attach_to_compose_service(project, service_name, compose_config)
                    .await?;
            }
        }

        Ok(())
    }

    /// Attach to a compose-managed service (stream logs, track status)
    async fn attach_to_compose_service(
        &mut self,
        project: &str,
        service_name: &str,
        config: &ComposeServiceConfig,
    ) -> Result<()> {
        let docker = match &self.docker {
            Some(d) => d,
            None => {
                self.log("Docker is not available".to_string());
                return Err(EnvibeError::DockerNotRunning);
            }
        };

        // Update state
        let mut state = ServiceState::new(service_name.to_string());
        state.status = ServiceStatus::Running;
        state.port = config.host_port;
        state.container_id = Some(config.container_id.clone());
        self.state.set_service_state(project, state);

        // Stream logs from the compose container
        docker.stream_logs(&config.container_id, service_name, self.log_tx.clone());

        let port_info = config
            .host_port
            .map(|p| format!(" on port {}", p))
            .unwrap_or_default();
        self.log(format!(
            "{} (compose){} - streaming logs",
            service_name, port_info
        ));

        Ok(())
    }

    async fn start_docker_service(
        &mut self,
        project: &str,
        service_name: &str,
        config: &DockerServiceConfig,
    ) -> Result<()> {
        let docker = match &self.docker {
            Some(d) => d,
            None => {
                self.log("Docker is not available".to_string());
                let mut state = ServiceState::new(service_name.to_string());
                state.status = ServiceStatus::Error;
                state.error_message = Some("Docker not available".to_string());
                self.state.set_service_state(project, state);
                return Err(EnvibeError::DockerNotRunning);
            }
        };

        // Allocate port
        let service_type = ServiceType::from_image(&config.image);
        let port = self.port_registry.get_or_allocate(
            project,
            service_name,
            &self.settings.port_ranges,
            service_type,
        )?;

        // Start container
        match docker.start_container(project, service_name, config, port).await {
            Ok(container_id) => {
                let mut state = ServiceState::new(service_name.to_string());
                state.status = ServiceStatus::Running;
                state.port = Some(port);
                state.container_id = Some(container_id.clone());
                self.state.set_service_state(project, state);

                // Start streaming logs
                docker.stream_logs(&container_id, service_name, self.log_tx.clone());

                self.log(format!("{} started on port {}", service_name, port));
            }
            Err(e) => {
                let mut state = ServiceState::new(service_name.to_string());
                state.status = ServiceStatus::Error;
                state.error_message = Some(e.to_string());
                self.state.set_service_state(project, state);

                self.log(format!("Failed to start {}: {}", service_name, e));
                return Err(e);
            }
        }

        Ok(())
    }

    async fn start_process_service(
        &mut self,
        project: &str,
        service_name: &str,
        config: &ProcessServiceConfig,
        project_path: &PathBuf,
        project_config: &ProjectConfig,
    ) -> Result<()> {
        // Allocate port for this process
        let service_type = ServiceType::from_service_name(service_name);
        let port = self.port_registry.get_or_allocate(
            project,
            service_name,
            &self.settings.port_ranges,
            service_type,
        )?;

        // Build service ports map for interpolation
        let mut service_ports: HashMap<String, u16> = HashMap::new();
        for (name, svc_config) in &project_config.services {
            // First check port registry
            if let Some(p) = self.port_registry.get_port(project, name) {
                service_ports.insert(name.clone(), p);
            }
            // Also include compose services with their host ports
            else if let ServiceConfig::Compose(compose_config) = svc_config {
                if let Some(p) = compose_config.host_port {
                    service_ports.insert(name.clone(), p);
                }
            }
        }
        service_ports.insert(service_name.to_string(), port);

        // Build environment variables from multiple sources
        // Priority (lowest to highest): project env-file, service env-file, service env
        let mut merged_env: HashMap<String, String> = HashMap::new();

        // 1. Load project-level env file
        if let Some(ref env_file) = project_config.env_file {
            let env_path = project_path.join(env_file);
            match parse_env_file(&env_path) {
                Ok(env) => {
                    merged_env.extend(env);
                }
                Err(e) => {
                    self.log(format!("Warning: Failed to load project env file: {}", e));
                }
            }
        }

        // 2. Load service-level env file
        if let Some(ref env_file) = config.env_file {
            let env_path = project_path.join(env_file);
            match parse_env_file(&env_path) {
                Ok(env) => {
                    merged_env.extend(env);
                }
                Err(e) => {
                    self.log(format!("Warning: Failed to load service env file: {}", e));
                }
            }
        }

        // 3. Apply service inline env (highest priority)
        merged_env.extend(config.env.clone());

        // Interpolate environment variables
        let env_vars = interpolate_env_map(&merged_env, &service_ports)?;

        match self
            .process_manager
            .start_process(
                project,
                service_name,
                config,
                project_path,
                env_vars,
                self.log_tx.clone(),
                None, // No shared state for TUI app
            )
            .await
        {
            Ok(pid) => {
                let mut state = ServiceState::new(service_name.to_string());
                state.status = ServiceStatus::Running;
                state.port = Some(port);
                state.process_id = Some(pid);
                self.state.set_service_state(project, state);

                self.log(format!("{} started (PID: {})", service_name, pid));
            }
            Err(e) => {
                let mut state = ServiceState::new(service_name.to_string());
                state.status = ServiceStatus::Error;
                state.error_message = Some(e.to_string());
                self.state.set_service_state(project, state);

                self.log(format!("Failed to start {}: {}", service_name, e));
                return Err(e);
            }
        }

        Ok(())
    }

    /// Stop a service
    async fn stop_service(
        &mut self,
        project: &str,
        service_name: &str,
        config: &ServiceConfig,
    ) -> Result<()> {
        self.log(format!("Stopping {}...", service_name));

        // Update state to stopping
        if let Some(state) = self.state.get_service_state_mut(project, service_name) {
            state.status = ServiceStatus::Stopping;
        }

        match config {
            ServiceConfig::Docker(_) => {
                self.stop_docker_service(project, service_name).await?;
            }
            ServiceConfig::Process(_) => {
                self.stop_process_service(project, service_name).await?;
            }
            ServiceConfig::Compose(_) => {
                // Compose services are externally managed - we can't stop them
                // Just update the UI state to indicate we're no longer tracking
                self.log(format!(
                    "{} is managed by docker compose - use 'docker compose down' to stop",
                    service_name
                ));
            }
        }

        Ok(())
    }

    async fn stop_docker_service(&mut self, project: &str, service_name: &str) -> Result<()> {
        let docker = match &self.docker {
            Some(d) => d,
            None => return Err(EnvibeError::DockerNotRunning),
        };

        // Get container ID from state
        let container_id = self
            .state
            .get_service_state(project, service_name)
            .and_then(|s| s.container_id.clone());

        if let Some(id) = container_id {
            docker.stop_container(&id).await?;
        }

        let mut state = ServiceState::new(service_name.to_string());
        state.status = ServiceStatus::Stopped;
        self.state.set_service_state(project, state);

        self.log(format!("{} stopped", service_name));
        Ok(())
    }

    async fn stop_process_service(&mut self, project: &str, service_name: &str) -> Result<()> {
        self.process_manager.stop_process(project, service_name).await?;

        let mut state = ServiceState::new(service_name.to_string());
        state.status = ServiceStatus::Stopped;
        self.state.set_service_state(project, state);

        self.log(format!("{} stopped", service_name));
        Ok(())
    }

    /// Start all services for current project
    pub async fn start_all_services(&mut self) -> Result<()> {
        let project = match self.current_project() {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        let config = match &project.config {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        self.log(format!("Starting all services for {}...", project.name));

        // Start services in dependency order (simple: compose first, then docker, then processes)
        let mut compose_services = Vec::new();
        let mut docker_services = Vec::new();
        let mut process_services = Vec::new();

        for (name, service_config) in &config.services {
            match service_config {
                ServiceConfig::Compose(_) => compose_services.push(name.clone()),
                ServiceConfig::Docker(_) => docker_services.push(name.clone()),
                ServiceConfig::Process(_) => process_services.push(name.clone()),
            }
        }

        // Attach to compose services first (already running)
        for name in compose_services {
            if let Some(service_config) = config.services.get(&name) {
                self.start_service(
                    &project.name,
                    &name,
                    service_config,
                    &project.path,
                    &config,
                )
                .await?;
            }
        }

        // Start docker services
        for name in docker_services {
            if let Some(service_config) = config.services.get(&name) {
                self.start_service(
                    &project.name,
                    &name,
                    service_config,
                    &project.path,
                    &config,
                )
                .await?;
            }
        }

        // Small delay for docker services to initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Then start process services
        for name in process_services {
            if let Some(service_config) = config.services.get(&name) {
                self.start_service(
                    &project.name,
                    &name,
                    service_config,
                    &project.path,
                    &config,
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Stop all services for current project
    pub async fn stop_all_services(&mut self) -> Result<()> {
        let project = match self.current_project() {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        let config = match &project.config {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        self.log(format!("Stopping all services for {}...", project.name));

        for (name, service_config) in &config.services {
            self.stop_service(&project.name, name, service_config)
                .await?;
        }

        Ok(())
    }

    /// Restart current service
    pub async fn restart_service(&mut self) -> Result<()> {
        let project = match self.current_project() {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        let config = match &project.config {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        let service_name = match self.current_service_name() {
            Some(n) => n,
            None => return Ok(()),
        };

        let service_config = match config.services.get(&service_name) {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        self.log(format!("Restarting {}...", service_name));

        self.stop_service(&project.name, &service_name, &service_config)
            .await?;

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        self.start_service(
            &project.name,
            &service_name,
            &service_config,
            &project.path,
            &config,
        )
        .await?;

        Ok(())
    }

    /// Add a log message
    pub fn log(&mut self, message: String) {
        let timestamp = chrono::Local::now().format("%H:%M:%S");
        self.logs
            .push(LogEntry::new(format!("[{}] {}", timestamp, message)));

        // Keep logs bounded
        if self.logs.len() > 10000 {
            self.logs.drain(0..1000);
        }
    }

    /// Process received log messages
    pub fn process_logs(&mut self) {
        while let Ok(msg) = self.log_rx.try_recv() {
            self.logs.push(LogEntry::new(msg));

            // Keep logs bounded
            if self.logs.len() > 10000 {
                self.logs.drain(0..1000);
            }
        }
    }

    /// Clear logs
    pub fn clear_logs(&mut self) {
        self.logs.clear();
        self.log_scroll = 0;
    }

    /// Toggle follow mode
    pub fn toggle_follow(&mut self) {
        self.follow_logs = !self.follow_logs;
    }

    /// Page down in console
    pub fn page_down(&mut self) {
        self.follow_logs = false;
        self.log_scroll = self.log_scroll.saturating_add(20);
    }

    /// Page up in console
    pub fn page_up(&mut self) {
        self.follow_logs = false;
        self.log_scroll = self.log_scroll.saturating_sub(20);
    }

    /// Save application state
    pub async fn save(&self) -> Result<()> {
        self.state.save(&self.data_dir).await?;
        self.port_registry.save(&self.data_dir).await?;
        Ok(())
    }

    /// Cleanup on shutdown
    pub async fn cleanup(&mut self) -> Result<()> {
        // Stop all processes
        self.process_manager.stop_all().await?;

        // Save state
        self.save().await?;

        Ok(())
    }
}
