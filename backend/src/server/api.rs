use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Path, State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tower_http::cors::{Any, CorsLayer};

use crate::agent::AgentManager;
use crate::config::{
    AppSettings, DockerServiceConfig, Project, ProjectConfig, ServiceConfig, ServiceState, ServiceStatus,
};
use crate::docker::DockerClient;
use crate::error::Result;
use crate::ports::{PortRegistry, ServiceType};
use crate::process::{interpolate_env_map, parse_env_file, ProcessManager};
use crate::state::{AppState, ProjectRegistry};

use super::ws::{handle_websocket, ws_terminal_handler};

/// Shared application state for the server
pub struct ServerState {
    pub projects: RwLock<Vec<Project>>,
    pub project_infos: RwLock<Vec<ProjectInfo>>,
    pub state: Arc<RwLock<AppState>>,
    pub port_registry: RwLock<PortRegistry>,
    pub project_registry: RwLock<ProjectRegistry>,
    pub settings: AppSettings,
    pub docker: Option<DockerClient>,
    pub data_dir: PathBuf,
    pub process_manager: RwLock<ProcessManager>,
    pub agent_manager: RwLock<AgentManager>,
    pub log_tx: mpsc::Sender<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    #[serde(rename = "hasDockerCompose")]
    pub has_docker_compose: bool,
    pub services: Vec<ServiceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub service_type: String,
    pub status: String,
    pub port: Option<u16>,
    #[serde(rename = "internalPort")]
    pub internal_port: Option<u16>,
    #[serde(rename = "containerId")]
    pub container_id: Option<String>,
    #[serde(rename = "processId")]
    pub process_id: Option<u32>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    pub image: Option<String>,
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogMessage {
    pub timestamp: String,
    pub level: String,
    pub service: Option<String>,
    pub project: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ServiceAction {
    pub project: String,
    pub service: String,
}

#[derive(Debug, Deserialize)]
pub struct SetPortAction {
    pub project: String,
    pub service: String,
    pub port: u16,
}

impl ProjectInfo {
    pub fn from_project(p: &Project, app_state: &AppState, port_registry: &PortRegistry) -> Self {
        let services = p
            .config
            .as_ref()
            .map(|c| {
                c.services
                    .iter()
                    .map(|(name, config)| {
                        let mut info = ServiceInfo::from_config(name, config);
                        // Update with runtime state
                        if let Some(state) = app_state.get_service_state(&p.name, name) {
                            info.update_from_state(state);
                        }
                        // If no port from state, check port registry for assigned port
                        if info.port.is_none() {
                            info.port = port_registry.get_port(&p.name, name);
                        }
                        info
                    })
                    .collect()
            })
            .unwrap_or_default();

        ProjectInfo {
            name: p.name.clone(),
            path: p.path.to_string_lossy().to_string(),
            has_docker_compose: p.has_docker_compose,
            services,
        }
    }
}

impl ServiceInfo {
    fn from_config(name: &str, config: &ServiceConfig) -> Self {
        match config {
            ServiceConfig::Docker(c) => ServiceInfo {
                name: name.to_string(),
                service_type: "docker".to_string(),
                status: "stopped".to_string(),
                port: None,
                internal_port: c.internal_port,
                container_id: None,
                process_id: None,
                error_message: None,
                image: Some(c.image.clone()),
                command: c.command.clone(),
            },
            ServiceConfig::Process(c) => ServiceInfo {
                name: name.to_string(),
                service_type: "process".to_string(),
                status: "stopped".to_string(),
                port: None,
                internal_port: None,
                container_id: None,
                process_id: None,
                error_message: None,
                image: None,
                command: Some(c.command.clone()),
            },
            ServiceConfig::Compose(c) => ServiceInfo {
                name: name.to_string(),
                service_type: "compose".to_string(),
                status: "running".to_string(),
                port: c.host_port,
                internal_port: c.internal_port,
                container_id: Some(c.container_id.clone()),
                process_id: None,
                error_message: None,
                image: Some(c.image.clone()),
                command: None,
            },
            ServiceConfig::Agent(c) => ServiceInfo {
                name: name.to_string(),
                service_type: "agent".to_string(),
                status: "stopped".to_string(),
                port: None,
                internal_port: None,
                container_id: None,
                process_id: None,
                error_message: None,
                image: None,
                command: Some(c.command.clone()),
            },
        }
    }

    fn update_from_state(&mut self, state: &ServiceState) {
        self.status = match state.status {
            ServiceStatus::Stopped => "stopped".to_string(),
            ServiceStatus::Starting => "starting".to_string(),
            ServiceStatus::Running => "running".to_string(),
            ServiceStatus::Stopping => "stopping".to_string(),
            ServiceStatus::Error => "error".to_string(),
        };
        if state.port.is_some() {
            self.port = state.port;
        }
        if state.container_id.is_some() {
            self.container_id = state.container_id.clone();
        }
        self.process_id = state.process_id;
        self.error_message = state.error_message.clone();
    }
}

pub async fn run_server(data_dir: PathBuf, port: u16) -> Result<()> {
    // Initialize state
    let state = AppState::load(&data_dir).await.unwrap_or_default();
    let port_registry = PortRegistry::load(&data_dir).await.unwrap_or_default();
    let project_registry = ProjectRegistry::load(&data_dir).await.unwrap_or_default();
    let docker = DockerClient::new().await.ok();
    let settings = AppSettings::default();

    let (log_tx, mut log_rx) = mpsc::channel::<String>(1000);

    // Spawn a task to forward service logs to stdout (Electron captures this)
    // Uses batching to reduce syscall overhead
    tokio::spawn(async move {
        use std::io::Write;
        let mut buffer = Vec::with_capacity(100);
        let flush_interval = tokio::time::Duration::from_millis(16); // ~60fps

        loop {
            // Wait for at least one log or timeout
            tokio::select! {
                Some(log) = log_rx.recv() => {
                    buffer.push(log);
                    // Drain any additional immediately available logs
                    while buffer.len() < 100 {
                        match log_rx.try_recv() {
                            Ok(log) => buffer.push(log),
                            Err(_) => break,
                        }
                    }
                }
                _ = tokio::time::sleep(flush_interval), if !buffer.is_empty() => {}
            }

            // Flush buffer to stdout
            if !buffer.is_empty() {
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                for log in buffer.drain(..) {
                    let _ = writeln!(handle, "{}", log);
                }
                let _ = handle.flush();
            }
        }
    });

    let server_state = Arc::new(ServerState {
        projects: RwLock::new(Vec::new()),
        project_infos: RwLock::new(Vec::new()),
        state: Arc::new(RwLock::new(state)),
        port_registry: RwLock::new(port_registry),
        project_registry: RwLock::new(project_registry),
        settings,
        docker,
        data_dir,
        process_manager: RwLock::new(ProcessManager::new()),
        agent_manager: RwLock::new(AgentManager::new()),
        log_tx,
    });

    // Scan for projects on startup
    scan_projects(server_state.clone()).await?;

    // Build router
    let app = Router::new()
        .route("/api/projects", get(get_projects))
        .route("/api/projects/:name/services", get(get_services))
        .route("/api/services/start", post(start_service))
        .route("/api/services/stop", post(stop_service))
        .route("/api/services/restart", post(restart_service))
        .route("/api/services/port", post(set_service_port))
        .route("/api/projects/add", post(add_project))
        .route("/api/projects/remove", post(remove_project))
        .route("/api/env/:project", get(get_env_vars))
        .route("/api/env/:project/:service", get(get_service_env_vars))
        .route("/ws", get(ws_handler))
        .route("/ws/terminal/:project/:service", get(ws_terminal_route))
        .route("/health", get(health_check))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(server_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("Starting server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn scan_projects(state: Arc<ServerState>) -> Result<()> {
    use crate::config::{
        detect_from_docker_compose, docker_compose_exists, get_config_path,
        merge_compose_services, parse_config, services_from_compose_containers,
    };

    // Load registered project paths
    let registry = state.project_registry.read().await;
    let registered_paths = registry.paths.clone();
    drop(registry);

    let mut projects = Vec::new();

    // Query running docker compose containers
    let compose_containers = if let Some(ref docker) = state.docker {
        let containers = docker.list_compose_containers().await.unwrap_or_default();
        tracing::debug!("Found {} compose containers", containers.len());
        containers
    } else {
        Vec::new()
    };

    // Query RUNNING envibe containers (started by envibe, not docker compose)
    let envibe_containers = if let Some(ref docker) = state.docker {
        docker.list_running_envibe_containers().await.unwrap_or_default()
    } else {
        Vec::new()
    };

    for path in &registered_paths {
        if !path.exists() || !path.is_dir() {
            tracing::warn!("Registered project path does not exist: {:?}", path);
            continue;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut project = Project::new(name.clone(), path.to_path_buf());

        // Check for .envibe.yaml
        if let Some(config_path) = get_config_path(path).await {
            match parse_config(&config_path).await {
                Ok(config) => {
                    tracing::debug!("Loaded config for {}: {:?}", name, config);
                    project.config = Some(config);
                }
                Err(e) => {
                    tracing::error!("Failed to parse config {}: {}", config_path.display(), e);
                }
            }
        }

        // Check for docker-compose.yml
        project.has_docker_compose = docker_compose_exists(path);

        // Always read docker-compose.yml if it exists and merge services
        if project.has_docker_compose {
            if let Ok(Some(compose_config)) = detect_from_docker_compose(path).await {
                if let Some(ref mut config) = project.config {
                    // Merge compose services into existing config
                    for (svc_name, svc_config) in compose_config.services {
                        // Only add if not already defined
                        if !config.services.contains_key(&svc_name) {
                            config.services.insert(svc_name, svc_config);
                        }
                    }
                } else {
                    project.config = Some(compose_config);
                }
            }
        }

        // Auto-discover running compose containers for this project
        let compose_services = services_from_compose_containers(&compose_containers, &name);

        if !compose_services.is_empty() {
            if let Some(ref mut config) = project.config {
                merge_compose_services(config, compose_services);
            } else {
                project.config = Some(ProjectConfig {
                    name: project.name.clone(),
                    env_file: None,
                    services: compose_services,
                    agents: HashMap::new(),
                });
            }
        }

        // Initialize/reset service states based on actual container status
        if let Some(ref config) = project.config {
            let mut app_state = state.state.write().await;

            // First, reset all Docker/Compose services to stopped (clear stale state)
            for (svc_name, svc_config) in &config.services {
                match svc_config {
                    ServiceConfig::Docker(_) | ServiceConfig::Compose(_) => {
                        let mut svc_state = ServiceState::new(svc_name.clone());
                        svc_state.status = ServiceStatus::Stopped;
                        svc_state.container_id = None;
                        app_state.set_service_state(&project.name, svc_state);
                    }
                    _ => {}
                }
            }

            // Then, mark actually running compose containers and stream their logs
            for (svc_name, svc_config) in &config.services {
                if let ServiceConfig::Compose(compose_config) = svc_config {
                    if compose_containers.iter().any(|c| c.project == name && c.service == *svc_name) {
                        let mut svc_state = ServiceState::new(svc_name.clone());
                        svc_state.status = ServiceStatus::Running;
                        svc_state.port = compose_config.host_port;
                        svc_state.container_id = Some(compose_config.container_id.clone());
                        app_state.set_service_state(&project.name, svc_state);

                        // Start streaming logs for already-running compose containers
                        if let Some(ref docker) = state.docker {
                            docker.stream_logs(&compose_config.container_id, svc_name, state.log_tx.clone());
                        }
                    }
                }
            }

            // Check for envibe-managed containers that are actually running
            for (container_id, project_name, service_name) in &envibe_containers {
                if project_name == &name && config.services.contains_key(service_name) {
                    let mut svc_state = ServiceState::new(service_name.clone());
                    svc_state.status = ServiceStatus::Running;
                    svc_state.container_id = Some(container_id.clone());
                    app_state.set_service_state(&name, svc_state);

                    // Start streaming logs for already-running envibe containers
                    if let Some(ref docker) = state.docker {
                        docker.stream_logs(container_id, service_name, state.log_tx.clone());
                    }
                }
            }
        }

        projects.push(project);
    }

    projects.sort_by(|a, b| a.name.cmp(&b.name));

    // Build project infos
    let app_state = state.state.read().await;
    let port_registry = state.port_registry.read().await;
    let project_infos: Vec<ProjectInfo> = projects
        .iter()
        .map(|p| ProjectInfo::from_project(p, &app_state, &port_registry))
        .collect();
    drop(port_registry);
    drop(app_state);

    let mut state_projects = state.projects.write().await;
    *state_projects = projects;
    drop(state_projects);

    let mut state_infos = state.project_infos.write().await;
    *state_infos = project_infos;

    Ok(())
}

async fn get_projects(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    // Rebuild infos with current state
    let projects = state.projects.read().await;
    let app_state = state.state.read().await;
    let port_registry = state.port_registry.read().await;
    let infos: Vec<ProjectInfo> = projects
        .iter()
        .map(|p| ProjectInfo::from_project(p, &app_state, &port_registry))
        .collect();
    Json(infos)
}

async fn get_services(
    State(state): State<Arc<ServerState>>,
    Path(project_name): Path<String>,
) -> impl IntoResponse {
    let projects = state.projects.read().await;
    let app_state = state.state.read().await;
    let port_registry = state.port_registry.read().await;
    let project = projects.iter().find(|p| p.name == project_name);

    match project {
        Some(p) => {
            let info = ProjectInfo::from_project(p, &app_state, &port_registry);
            Json(info.services).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Project not found").into_response(),
    }
}

async fn start_service(
    State(state): State<Arc<ServerState>>,
    Json(action): Json<ServiceAction>,
) -> impl IntoResponse {
    tracing::info!("Starting service: {}/{}", action.project, action.service);

    let projects = state.projects.read().await;
    let project = match projects.iter().find(|p| p.name == action.project) {
        Some(p) => p.clone(),
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };
    drop(projects);

    let config = match &project.config {
        Some(c) => c.clone(),
        None => return (StatusCode::NOT_FOUND, "Project has no config").into_response(),
    };

    let service_config = match config.services.get(&action.service) {
        Some(c) => c.clone(),
        None => return (StatusCode::NOT_FOUND, "Service not found").into_response(),
    };

    match service_config {
        ServiceConfig::Process(process_config) => {
            // Allocate port
            let mut port_registry = state.port_registry.write().await;
            let service_type = ServiceType::from_service_name(&action.service);
            let port = match port_registry.get_or_allocate(
                &action.project,
                &action.service,
                &state.settings.port_ranges,
                service_type,
            ) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Failed to allocate port: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to allocate port").into_response();
                }
            };
            drop(port_registry);

            // Build service ports map
            let mut service_ports: HashMap<String, u16> = HashMap::new();
            let app_state = state.state.read().await;
            for (name, svc_config) in &config.services {
                if let Some(svc_state) = app_state.get_service_state(&action.project, name) {
                    if let Some(p) = svc_state.port {
                        service_ports.insert(name.clone(), p);
                    }
                }
                if let ServiceConfig::Compose(compose_config) = svc_config {
                    if let Some(p) = compose_config.host_port {
                        service_ports.insert(name.clone(), p);
                    }
                }
            }
            drop(app_state);
            service_ports.insert(action.service.clone(), port);

            // Build environment variables
            let mut merged_env: HashMap<String, String> = HashMap::new();

            // Load project-level env file
            if let Some(ref env_file) = config.env_file {
                let env_path = project.path.join(env_file);
                if let Ok(env) = parse_env_file(&env_path) {
                    merged_env.extend(env);
                }
            }

            // Load service-level env file
            if let Some(ref env_file) = process_config.env_file {
                let env_path = project.path.join(env_file);
                if let Ok(env) = parse_env_file(&env_path) {
                    merged_env.extend(env);
                }
            }

            // Apply service inline env
            merged_env.extend(process_config.env.clone());

            // Interpolate
            let env_vars = match interpolate_env_map(&merged_env, &service_ports) {
                Ok(env) => env,
                Err(e) => {
                    tracing::error!("Failed to interpolate env vars: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to interpolate env vars").into_response();
                }
            };

            // Start the process
            let mut process_manager = state.process_manager.write().await;
            let app_state_clone = Arc::clone(&state.state);
            match process_manager.start_process(
                &action.project,
                &action.service,
                &process_config,
                &project.path,
                env_vars,
                state.log_tx.clone(),
                Some(app_state_clone),
            ).await {
                Ok(pid) => {
                    tracing::info!("Started {} with PID {}", action.service, pid);

                    // Update state
                    let mut app_state = state.state.write().await;
                    let mut svc_state = ServiceState::new(action.service.clone());
                    svc_state.status = ServiceStatus::Running;
                    svc_state.port = Some(port);
                    svc_state.process_id = Some(pid);
                    app_state.set_service_state(&action.project, svc_state);

                    Json(serde_json::json!({ "status": "started", "pid": pid, "port": port })).into_response()
                }
                Err(e) => {
                    tracing::error!("Failed to start service: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to start: {}", e)).into_response()
                }
            }
        }
        ServiceConfig::Compose(compose_config) => {
            // For compose services, we start the container using Docker
            let docker = match &state.docker {
                Some(d) => d,
                None => return (StatusCode::SERVICE_UNAVAILABLE, "Docker not available").into_response(),
            };

            // Get port from registry first, fall back to compose config
            let mut port_registry = state.port_registry.write().await;
            let service_type = ServiceType::from_service_name(&action.service);
            let port = match port_registry.get_or_allocate(
                &action.project,
                &action.service,
                &state.settings.port_ranges,
                service_type,
            ) {
                Ok(p) => p,
                Err(_) => compose_config.host_port.unwrap_or(0),
            };
            drop(port_registry);

            let docker_config = DockerServiceConfig {
                image: compose_config.image.clone(),
                env: compose_config.env.clone(),
                volumes: compose_config.volumes.clone(),
                ports: Vec::new(),
                command: None,
                depends_on: Vec::new(),
                internal_port: compose_config.internal_port,
            };

            // start_container handles checking port changes and recreating if needed
            match docker.start_container(&action.project, &action.service, &docker_config, port).await {
                Ok(container_id) => {
                    tracing::info!("Started container {} for {} on port {}", container_id, action.service, port);

                    // Stream container logs to the log channel
                    docker.stream_logs(&container_id, &action.service, state.log_tx.clone());

                    let mut app_state = state.state.write().await;
                    let mut svc_state = ServiceState::new(action.service.clone());
                    svc_state.status = ServiceStatus::Running;
                    svc_state.container_id = Some(container_id.clone());
                    svc_state.port = Some(port);
                    app_state.set_service_state(&action.project, svc_state);

                    Json(serde_json::json!({ "status": "started", "container_id": container_id, "port": port })).into_response()
                }
                Err(e) => {
                    tracing::error!("Failed to start container: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to start: {}", e)).into_response()
                }
            }
        }
        ServiceConfig::Docker(docker_config) => {
            let docker = match &state.docker {
                Some(d) => d,
                None => return (StatusCode::SERVICE_UNAVAILABLE, "Docker not available").into_response(),
            };

            // Allocate port
            let mut port_registry = state.port_registry.write().await;
            let service_type = ServiceType::from_service_name(&action.service);
            let port = match port_registry.get_or_allocate(
                &action.project,
                &action.service,
                &state.settings.port_ranges,
                service_type,
            ) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Failed to allocate port: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to allocate port").into_response();
                }
            };
            drop(port_registry);

            match docker.start_container(&action.project, &action.service, &docker_config, port).await {
                Ok(container_id) => {
                    tracing::info!("Started Docker container {} for {}", container_id, action.service);

                    // Stream container logs to the log channel
                    docker.stream_logs(&container_id, &action.service, state.log_tx.clone());

                    let mut app_state = state.state.write().await;
                    let mut svc_state = ServiceState::new(action.service.clone());
                    svc_state.status = ServiceStatus::Running;
                    svc_state.container_id = Some(container_id.clone());
                    svc_state.port = Some(port);
                    app_state.set_service_state(&action.project, svc_state);

                    Json(serde_json::json!({ "status": "started", "container_id": container_id, "port": port })).into_response()
                }
                Err(e) => {
                    tracing::error!("Failed to start Docker service: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to start: {}", e)).into_response()
                }
            }
        }
        ServiceConfig::Agent(agent_config) => {
            let key = format!("{}:{}", action.project, action.service);

            // Update state to starting
            {
                let mut app_state = state.state.write().await;
                let mut svc_state = ServiceState::new(action.service.clone());
                svc_state.status = ServiceStatus::Starting;
                app_state.set_service_state(&action.project, svc_state);
            }

            let mut agent_manager = state.agent_manager.write().await;
            match agent_manager.start_agent(
                key,
                &agent_config,
                &project.path,
                state.log_tx.clone(),
                action.project.clone(),
                action.service.clone(),
            ).await {
                Ok(pid) => {
                    tracing::info!("Started agent {} with PID {}", action.service, pid);

                    let mut app_state = state.state.write().await;
                    let mut svc_state = ServiceState::new(action.service.clone());
                    svc_state.status = ServiceStatus::Running;
                    svc_state.process_id = Some(pid);
                    app_state.set_service_state(&action.project, svc_state);

                    Json(serde_json::json!({ "status": "started", "pid": pid })).into_response()
                }
                Err(e) => {
                    tracing::error!("Failed to start agent: {}", e);

                    let mut app_state = state.state.write().await;
                    let mut svc_state = ServiceState::new(action.service.clone());
                    svc_state.status = ServiceStatus::Error;
                    svc_state.error_message = Some(format!("{}", e));
                    app_state.set_service_state(&action.project, svc_state);

                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to start agent: {}", e)).into_response()
                }
            }
        }
    }
}

async fn stop_service(
    State(state): State<Arc<ServerState>>,
    Json(action): Json<ServiceAction>,
) -> impl IntoResponse {
    tracing::info!("Stopping service: {}/{}", action.project, action.service);

    // Find the project and service config
    let projects = state.projects.read().await;
    let project = projects.iter().find(|p| p.name == action.project);

    let service_config = project
        .and_then(|p| p.config.as_ref())
        .and_then(|c| c.services.get(&action.service));

    match service_config {
        Some(ServiceConfig::Process(_)) => {
            let mut process_manager = state.process_manager.write().await;
            if let Err(e) = process_manager.stop_process(&action.project, &action.service).await {
                tracing::error!("Failed to stop process: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to stop: {}", e)).into_response();
            }
        }
        Some(ServiceConfig::Docker(_)) | Some(ServiceConfig::Compose(_)) => {
            // Get container ID from state
            let app_state = state.state.read().await;
            let container_id = app_state
                .get_service_state(&action.project, &action.service)
                .and_then(|s| s.container_id.clone());
            drop(app_state);

            if let Some(container_id) = container_id {
                if let Some(ref docker) = state.docker {
                    if let Err(e) = docker.stop_container(&container_id).await {
                        tracing::error!("Failed to stop container: {}", e);
                        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to stop container: {}", e)).into_response();
                    }
                    tracing::info!("Stopped container {}", container_id);
                } else {
                    return (StatusCode::SERVICE_UNAVAILABLE, "Docker not available").into_response();
                }
            } else {
                // Try to find container by name pattern
                let container_name = format!("{}_{}", action.project, action.service);
                if let Some(ref docker) = state.docker {
                    // Try different naming conventions
                    let names_to_try = vec![
                        container_name.clone(),
                        format!("{}-{}-1", action.project, action.service),
                    ];

                    let mut stopped = false;
                    for name in names_to_try {
                        if let Ok(containers) = docker.list_envibe_containers().await {
                            for (id, proj, svc) in containers {
                                if proj == action.project && svc == action.service {
                                    if let Err(e) = docker.stop_container(&id).await {
                                        tracing::error!("Failed to stop container {}: {}", id, e);
                                    } else {
                                        tracing::info!("Stopped container {} by ID", id);
                                        stopped = true;
                                        break;
                                    }
                                }
                            }
                        }
                        if stopped { break; }
                    }

                    if !stopped {
                        tracing::warn!("No container found for {}/{}", action.project, action.service);
                    }
                }
            }
        }
        Some(ServiceConfig::Agent(_)) => {
            let key = format!("{}:{}", action.project, action.service);
            let mut agent_manager = state.agent_manager.write().await;
            if let Err(e) = agent_manager.stop_agent(&key) {
                tracing::error!("Failed to stop agent: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to stop agent: {}", e)).into_response();
            }
        }
        None => {
            // Service not found in config, try process manager anyway
            let mut process_manager = state.process_manager.write().await;
            let _ = process_manager.stop_process(&action.project, &action.service).await;
        }
    }

    // Update state
    let mut app_state = state.state.write().await;
    let mut svc_state = ServiceState::new(action.service.clone());
    svc_state.status = ServiceStatus::Stopped;
    svc_state.container_id = None;
    app_state.set_service_state(&action.project, svc_state);

    Json(serde_json::json!({ "status": "stopped" })).into_response()
}

async fn restart_service(
    State(state): State<Arc<ServerState>>,
    Json(action): Json<ServiceAction>,
) -> impl IntoResponse {
    tracing::info!("Restarting service: {}/{}", action.project, action.service);

    // Stop first — check service type for agent
    let is_agent = {
        let projects = state.projects.read().await;
        projects.iter().find(|p| p.name == action.project)
            .and_then(|p| p.config.as_ref())
            .and_then(|c| c.services.get(&action.service))
            .is_some_and(|c| matches!(c, ServiceConfig::Agent(_)))
    };

    if is_agent {
        let key = format!("{}:{}", action.project, action.service);
        let mut agent_manager = state.agent_manager.write().await;
        let _ = agent_manager.stop_agent(&key);
    } else {
        let mut process_manager = state.process_manager.write().await;
        let _ = process_manager.stop_process(&action.project, &action.service).await;
    }

    // Small delay
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Start again (delegate to start_service logic)
    start_service(State(state), Json(action)).await
}

async fn set_service_port(
    State(state): State<Arc<ServerState>>,
    Json(action): Json<SetPortAction>,
) -> impl IntoResponse {
    tracing::info!("Setting port for {}/{} to {}", action.project, action.service, action.port);

    // Validate port range
    if action.port < 1024 || action.port > 65535 {
        return (StatusCode::BAD_REQUEST, "Port must be between 1024 and 65535").into_response();
    }

    // Set the port in the registry
    let mut port_registry = state.port_registry.write().await;
    match port_registry.set_port(&action.project, &action.service, action.port) {
        Ok(_) => {
            // Save the registry
            if let Err(e) = port_registry.save(&state.data_dir).await {
                tracing::error!("Failed to save port registry: {}", e);
            }
            drop(port_registry);

            // Update the service state with the new port
            let mut app_state = state.state.write().await;
            if let Some(svc_state) = app_state.get_service_state(&action.project, &action.service) {
                let mut new_state = svc_state.clone();
                new_state.port = Some(action.port);
                app_state.set_service_state(&action.project, new_state);
            }

            Json(serde_json::json!({
                "status": "ok",
                "port": action.port
            })).into_response()
        }
        Err(e) => {
            (StatusCode::CONFLICT, format!("{}", e)).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct ProjectPathAction {
    path: String,
}

async fn add_project(
    State(state): State<Arc<ServerState>>,
    Json(action): Json<ProjectPathAction>,
) -> impl IntoResponse {
    let path = PathBuf::from(&action.path);
    if !path.exists() || !path.is_dir() {
        return (StatusCode::BAD_REQUEST, "Path does not exist or is not a directory").into_response();
    }

    let mut registry = state.project_registry.write().await;
    if !registry.add(path) {
        return (StatusCode::CONFLICT, "Project already added").into_response();
    }
    if let Err(e) = registry.save(&state.data_dir).await {
        tracing::error!("Failed to save project registry: {}", e);
    }
    drop(registry);

    // Re-scan to pick up the new project
    if let Err(e) = scan_projects(state.clone()).await {
        tracing::error!("Failed to rescan projects: {}", e);
    }

    Json(serde_json::json!({ "status": "added" })).into_response()
}

async fn remove_project(
    State(state): State<Arc<ServerState>>,
    Json(action): Json<ProjectPathAction>,
) -> impl IntoResponse {
    let path = PathBuf::from(&action.path);

    let mut registry = state.project_registry.write().await;
    if !registry.remove(&path) {
        return (StatusCode::NOT_FOUND, "Project not in registry").into_response();
    }
    if let Err(e) = registry.save(&state.data_dir).await {
        tracing::error!("Failed to save project registry: {}", e);
    }
    drop(registry);

    // Re-scan to remove the project from in-memory state
    if let Err(e) = scan_projects(state.clone()).await {
        tracing::error!("Failed to rescan projects: {}", e);
    }

    Json(serde_json::json!({ "status": "removed" })).into_response()
}

async fn get_env_vars(
    State(state): State<Arc<ServerState>>,
    Path(project_name): Path<String>,
) -> impl IntoResponse {
    let projects = state.projects.read().await;
    let project = match projects.iter().find(|p| p.name == project_name) {
        Some(p) => p,
        None => return Json(HashMap::<String, String>::new()),
    };

    let mut env_vars: HashMap<String, String> = HashMap::new();

    // Load project-level env file
    if let Some(ref config) = project.config {
        if let Some(ref env_file) = config.env_file {
            let env_path = project.path.join(env_file);
            if let Ok(env) = parse_env_file(&env_path) {
                env_vars.extend(env);
            }
        }
    }

    // Interpolate with known service ports
    let port_registry = state.port_registry.read().await;
    let service_ports: HashMap<String, u16> = port_registry
        .get_project_ports(&project_name)
        .into_iter()
        .collect();
    drop(port_registry);

    if let Ok(interpolated) = interpolate_env_map(&env_vars, &service_ports) {
        return Json(interpolated);
    }

    Json(env_vars)
}

async fn get_service_env_vars(
    State(state): State<Arc<ServerState>>,
    Path((project_name, service_name)): Path<(String, String)>,
) -> impl IntoResponse {
    tracing::debug!("get_service_env_vars: project={}, service={}", project_name, service_name);

    let projects = state.projects.read().await;
    let project = match projects.iter().find(|p| p.name == project_name) {
        Some(p) => p,
        None => {
            tracing::debug!("Project not found: {}", project_name);
            return Json(HashMap::<String, String>::new());
        }
    };

    let mut env_vars: HashMap<String, String> = HashMap::new();

    if let Some(ref config) = project.config {
        tracing::debug!("Project config found, services: {:?}", config.services.keys().collect::<Vec<_>>());

        // Load project-level env file
        if let Some(ref env_file) = config.env_file {
            let env_path = project.path.join(env_file);
            tracing::debug!("Loading project env file: {}", env_path.display());
            if let Ok(env) = parse_env_file(&env_path) {
                tracing::debug!("Loaded {} project env vars", env.len());
                env_vars.extend(env);
            }
        }

        // Load service-specific env
        if let Some(service_config) = config.services.get(&service_name) {
            tracing::debug!("Service config found for: {}", service_name);
            if let ServiceConfig::Process(process_config) = service_config {
                // Load service-level env file
                if let Some(ref env_file) = process_config.env_file {
                    let env_path = project.path.join(env_file);
                    tracing::debug!("Loading service env file: {}", env_path.display());
                    match parse_env_file(&env_path) {
                        Ok(env) => {
                            tracing::debug!("Loaded {} service env vars", env.len());
                            env_vars.extend(env);
                        }
                        Err(e) => {
                            tracing::error!("Failed to parse env file {}: {}", env_path.display(), e);
                        }
                    }
                } else {
                    tracing::debug!("No env_file configured for service");
                }
                // Apply inline env vars
                env_vars.extend(process_config.env.clone());
            } else if let ServiceConfig::Docker(docker_config) = service_config {
                // Apply docker inline env vars
                env_vars.extend(docker_config.env.clone());
            }
        } else {
            tracing::debug!("Service not found in config: {}", service_name);
        }
    } else {
        tracing::debug!("No config for project: {}", project_name);
    }

    // Interpolate with known service ports
    let port_registry = state.port_registry.read().await;
    let service_ports: HashMap<String, u16> = port_registry
        .get_project_ports(&project_name)
        .into_iter()
        .collect();
    drop(port_registry);

    if let Ok(interpolated) = interpolate_env_map(&env_vars, &service_ports) {
        return Json(interpolated);
    }

    Json(env_vars)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

async fn ws_terminal_route(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ServerState>>,
    Path((project, service)): Path<(String, String)>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_terminal_handler(socket, state, project, service))
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
