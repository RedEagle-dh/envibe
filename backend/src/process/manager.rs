use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

#[cfg(unix)]
use nix::libc;
use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, RwLock};

use crate::config::{ProcessServiceConfig, ServiceStatus};
use crate::error::{EnvibeError, Result};
use crate::state::AppState;

/// Parse an env file and return a HashMap of environment variables
/// Supports standard .env format: KEY=value, with # comments
pub fn parse_env_file(path: &Path) -> Result<HashMap<String, String>> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        EnvibeError::Config(format!("Failed to read env file {}: {}", path.display(), e))
    })?;

    let mut env = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse KEY=value
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let mut value = value.trim();

            // Remove surrounding quotes if present
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value = &value[1..value.len() - 1];
            }

            if !key.is_empty() {
                env.insert(key.to_string(), value.to_string());
            }
        }
    }

    Ok(env)
}

pub struct ProcessManager {
    processes: HashMap<String, ManagedProcess>,
}

struct ManagedProcess {
    pid: u32,
    _log_handle: tokio::task::JoinHandle<()>,
    _stderr_handle: tokio::task::JoinHandle<()>,
    _exit_handle: tokio::task::JoinHandle<()>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    /// Start a process for a service
    /// `app_state` is optional - if provided, the exit monitor will update the state when the process exits
    pub async fn start_process(
        &mut self,
        project: &str,
        service_name: &str,
        config: &ProcessServiceConfig,
        project_path: &PathBuf,
        env_vars: HashMap<String, String>,
        log_tx: mpsc::Sender<String>,
        app_state: Option<Arc<RwLock<AppState>>>,
    ) -> Result<u32> {
        let key = format!("{}:{}", project, service_name);

        // Check if already running
        #[cfg(unix)]
        {
            let should_remove = if let Some(proc) = self.processes.get(&key) {
                // Check if process is still alive using signal 0
                let is_alive = unsafe { libc::kill(proc.pid as i32, 0) == 0 };
                if is_alive {
                    // Still running
                    return Ok(proc.pid);
                } else {
                    // Process exited, need to remove it
                    true
                }
            } else {
                false
            };

            if should_remove {
                self.processes.remove(&key);
            }
        }

        #[cfg(not(unix))]
        {
            if let Some(proc) = self.processes.get(&key) {
                return Ok(proc.pid);
            }
        }

        // Resolve working directory
        let working_dir = if config.working_dir == "." {
            project_path.clone()
        } else {
            project_path.join(&config.working_dir)
        };

        tracing::info!(
            "Starting process in {}: {}",
            working_dir.display(),
            config.command
        );

        // Run through shell to handle complex commands properly
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

        let mut cmd = Command::new(&shell);
        cmd.arg("-c")
            .arg(&config.command)
            .current_dir(&working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Create a new process group so we can kill all children
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        // Inherit PATH and other important env vars from parent
        if let Ok(path) = std::env::var("PATH") {
            cmd.env("PATH", path);
        }
        if let Ok(home) = std::env::var("HOME") {
            cmd.env("HOME", home);
        }
        if let Ok(user) = std::env::var("USER") {
            cmd.env("USER", user);
        }
        // For Node.js tools
        if let Ok(node_path) = std::env::var("NODE_PATH") {
            cmd.env("NODE_PATH", node_path);
        }
        // For bun
        if let Ok(bun_install) = std::env::var("BUN_INSTALL") {
            cmd.env("BUN_INSTALL", bun_install);
        }

        // Set custom environment variables
        for (k, v) in env_vars {
            cmd.env(&k, &v);
        }

        // Also add config-defined env vars
        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        // Set TERM for proper output handling
        cmd.env("TERM", "xterm-256color");
        cmd.env("FORCE_COLOR", "1");

        let mut child = cmd.spawn().map_err(|e| {
            EnvibeError::Process(format!("Failed to spawn process '{}': {}", config.command, e))
        })?;

        let pid = child.id().unwrap_or(0);
        tracing::info!("Process spawned with PID {}", pid);

        // Capture stdout
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let service_name_clone = service_name.to_string();
        let log_tx_clone = log_tx.clone();

        let log_handle = tokio::spawn(async move {
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let formatted = format!("[{}] {}", service_name_clone, line);
                    if log_tx_clone.send(formatted).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Also capture stderr in a separate task
        let service_name_clone2 = service_name.to_string();
        let log_tx_stderr = log_tx.clone();
        let stderr_handle = tokio::spawn(async move {
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let formatted = format!("[{} ERR] {}", service_name_clone2, line);
                    if log_tx_stderr.send(formatted).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Monitor process exit and send notification
        let service_name_exit = service_name.to_string();
        let project_exit = project.to_string();
        let exit_handle = tokio::spawn(async move {
            let exit_status = child.wait().await;
            let (exit_msg, is_error) = match &exit_status {
                Ok(status) => {
                    if status.success() {
                        (format!("[{} EXIT] Process exited successfully", service_name_exit), false)
                    } else {
                        let code = status.code().map(|c| c.to_string()).unwrap_or_else(|| "unknown".to_string());
                        (format!("[{} EXIT] Process exited with code {}", service_name_exit, code), true)
                    }
                }
                Err(e) => (format!("[{} EXIT] Process error: {}", service_name_exit, e), true),
            };

            // Update the app state immediately (if provided)
            if let Some(ref state_arc) = app_state {
                use crate::config::ServiceState;
                let mut state = state_arc.write().await;
                let mut svc_state = ServiceState::new(service_name_exit.clone());
                svc_state.status = if is_error { ServiceStatus::Error } else { ServiceStatus::Stopped };
                state.set_service_state(&project_exit, svc_state);
                tracing::info!("Updated state for {}/{} to {:?}", project_exit, service_name_exit,
                    if is_error { "error" } else { "stopped" });
            }

            // Send exit notification to frontend
            let _ = log_tx.send(exit_msg).await;
            // Also send a special status update message for the frontend
            let status_str = if is_error { "error" } else { "stopped" };
            let _ = log_tx.send(format!("[__STATUS__] project={} service={} status={}", project_exit, service_name_exit, status_str)).await;
        });

        self.processes.insert(
            key,
            ManagedProcess {
                pid,
                _log_handle: log_handle,
                _stderr_handle: stderr_handle,
                _exit_handle: exit_handle,
            },
        );

        Ok(pid)
    }

    /// Stop a process and all its children (kills entire process group)
    pub async fn stop_process(&mut self, project: &str, service_name: &str) -> Result<()> {
        let key = format!("{}:{}", project, service_name);

        if let Some(proc) = self.processes.remove(&key) {
            tracing::info!("Stopping process group {} for {}", proc.pid, key);

            #[cfg(unix)]
            {
                use nix::sys::signal::{self, Signal};
                use nix::unistd::Pid;

                // Kill the entire process group (negative PID)
                // The process group ID is the same as the leader's PID since we used process_group(0)
                let pgid = Pid::from_raw(-(proc.pid as i32));

                // Try graceful shutdown first
                if signal::kill(pgid, Signal::SIGTERM).is_err() {
                    // If process group kill fails, try killing just the process
                    let _ = signal::kill(Pid::from_raw(proc.pid as i32), Signal::SIGTERM);
                }

                // Wait for graceful shutdown
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // Check if any process in the group is still alive
                let is_alive = unsafe { libc::kill(-(proc.pid as i32), 0) == 0 };
                if is_alive {
                    tracing::warn!("Process group {} did not terminate, force killing", proc.pid);
                    let _ = signal::kill(pgid, Signal::SIGKILL);
                    // Also try killing just the main process
                    let _ = signal::kill(Pid::from_raw(proc.pid as i32), Signal::SIGKILL);
                }
            }

            #[cfg(not(unix))]
            {
                // On Windows, kill the process tree
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &proc.pid.to_string(), "/T", "/F"])
                    .output();
            }
        }

        Ok(())
    }

    /// Check if a process is running
    #[cfg(unix)]
    pub fn is_running(&mut self, project: &str, service_name: &str) -> bool {
        let key = format!("{}:{}", project, service_name);

        if let Some(proc) = self.processes.get(&key) {
            // Check if process is still alive by sending signal 0
            unsafe { libc::kill(proc.pid as i32, 0) == 0 }
        } else {
            false
        }
    }

    #[cfg(not(unix))]
    pub fn is_running(&mut self, project: &str, service_name: &str) -> bool {
        let key = format!("{}:{}", project, service_name);
        self.processes.contains_key(&key)
    }

    /// Get process status
    pub fn get_status(&mut self, project: &str, service_name: &str) -> ServiceStatus {
        if self.is_running(project, service_name) {
            ServiceStatus::Running
        } else {
            ServiceStatus::Stopped
        }
    }

    /// Stop all processes
    pub async fn stop_all(&mut self) -> Result<()> {
        let keys: Vec<String> = self.processes.keys().cloned().collect();
        for key in keys {
            if let Some((project, service)) = key.split_once(':') {
                self.stop_process(project, service).await?;
            }
        }
        Ok(())
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Interpolate environment variables in a string
/// Supports ${service.port}, ${service.host}, etc.
pub fn interpolate_env(
    value: &str,
    service_ports: &HashMap<String, u16>,
) -> Result<String> {
    let re = Regex::new(r"\$\{([^}]+)\}").unwrap();

    let mut result = value.to_string();
    let mut replacements = Vec::new();

    for cap in re.captures_iter(value) {
        let full_match = cap.get(0).unwrap().as_str();
        let var_name = cap.get(1).unwrap().as_str();

        let replacement = if let Some((service, prop)) = var_name.split_once('.') {
            match prop {
                "port" => service_ports
                    .get(service)
                    .map(|p| p.to_string())
                    .ok_or_else(|| {
                        EnvibeError::Interpolation(format!(
                            "Service '{}' not found for port interpolation",
                            service
                        ))
                    })?,
                "host" => "localhost".to_string(),
                "url" => {
                    let port = service_ports.get(service).ok_or_else(|| {
                        EnvibeError::Interpolation(format!(
                            "Service '{}' not found for URL interpolation",
                            service
                        ))
                    })?;
                    format!("localhost:{}", port)
                }
                _ => {
                    return Err(EnvibeError::Interpolation(format!(
                        "Unknown property '{}' for service '{}'",
                        prop, service
                    )))
                }
            }
        } else {
            // Check environment variable
            std::env::var(var_name).unwrap_or_else(|_| full_match.to_string())
        };

        replacements.push((full_match.to_string(), replacement));
    }

    for (from, to) in replacements {
        result = result.replace(&from, &to);
    }

    Ok(result)
}

/// Interpolate all environment variables in a HashMap
pub fn interpolate_env_map(
    env: &HashMap<String, String>,
    service_ports: &HashMap<String, u16>,
) -> Result<HashMap<String, String>> {
    let mut result = HashMap::new();
    for (k, v) in env {
        result.insert(k.clone(), interpolate_env(v, service_ports)?);
    }
    Ok(result)
}
