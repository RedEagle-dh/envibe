use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn};

use crate::config::types::AgentServiceConfig;
use crate::error::Result;

/// Maximum bytes to buffer for late-connecting WebSocket clients
const OUTPUT_HISTORY_MAX: usize = 128 * 1024; // 128KB

struct ManagedAgent {
    /// Broadcast sender for PTY output → WebSocket clients
    output_tx: broadcast::Sender<Vec<u8>>,
    /// Rolling buffer of recent PTY output (for late subscribers)
    output_history: Arc<Mutex<Vec<u8>>>,
    /// PTY master writer (wrapped in Arc<Mutex> for thread-safe access)
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// PTY master (kept alive to allow resize)
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    /// Handle to the reader task
    _reader_handle: tokio::task::JoinHandle<()>,
    /// Handle to the exit monitor task
    _exit_handle: tokio::task::JoinHandle<()>,
}

pub struct AgentManager {
    agents: HashMap<String, ManagedAgent>,
}

impl AgentManager {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    pub async fn start_agent(
        &mut self,
        key: String,
        config: &AgentServiceConfig,
        project_path: &Path,
        log_tx: mpsc::Sender<String>,
        project_name: String,
        service_name: String,
    ) -> Result<u32> {
        if self.agents.contains_key(&key) {
            return Err(crate::error::EnvibeError::Process(
                format!("Agent {} is already running", key),
            ));
        }

        let pty_system = native_pty_system();

        let pty_size = PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system.openpty(pty_size).map_err(|e| {
            crate::error::EnvibeError::Process(format!("Failed to open PTY: {}", e))
        })?;

        // Build the command
        let working_dir = if config.working_dir == "." {
            project_path.to_path_buf()
        } else if Path::new(&config.working_dir).is_absolute() {
            std::path::PathBuf::from(&config.working_dir)
        } else {
            project_path.join(&config.working_dir)
        };

        let mut cmd = CommandBuilder::new(&config.command);
        for arg in &config.args {
            cmd.arg(arg);
        }
        cmd.cwd(&working_dir);

        // Set environment
        for (k, v) in &config.env {
            cmd.env(k, v);
        }
        cmd.env("TERM", "xterm-256color");

        // Spawn the child process on the slave PTY
        let child = pair.slave.spawn_command(cmd).map_err(|e| {
            crate::error::EnvibeError::Process(format!("Failed to spawn agent: {}", e))
        })?;

        let child_pid = child.process_id().unwrap_or(0);
        info!("Started agent {} (PID: {})", key, child_pid);

        // Get writer and reader from master
        let writer = pair.master.take_writer().map_err(|e| {
            crate::error::EnvibeError::Process(format!("Failed to get PTY writer: {}", e))
        })?;
        let reader = pair.master.try_clone_reader().map_err(|e| {
            crate::error::EnvibeError::Process(format!("Failed to get PTY reader: {}", e))
        })?;

        // Broadcast channel for live WebSocket subscribers
        let (output_tx, _) = broadcast::channel(256);

        // Output history buffer for late-connecting clients
        let output_history: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::with_capacity(8192)));

        let writer = Arc::new(Mutex::new(writer));
        let master = Arc::new(Mutex::new(pair.master));

        // Spawn reader task: reads PTY output → history buffer + broadcast channel.
        // Agent output is NOT forwarded to the log panel — it contains raw terminal
        // escape sequences that only make sense in xterm.js. The log panel only
        // receives lifecycle events (started/exited) from the exit monitor task.
        let tx_clone = output_tx.clone();
        let history_clone = output_history.clone();
        let key_clone = key.clone();
        let reader_handle = tokio::task::spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        debug!("PTY reader EOF for agent {}", key_clone);
                        break;
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();

                        // Append to history buffer (cap at max size)
                        if let Ok(mut history) = history_clone.lock() {
                            history.extend_from_slice(&data);
                            if history.len() > OUTPUT_HISTORY_MAX {
                                let drain_to = history.len() - OUTPUT_HISTORY_MAX;
                                history.drain(..drain_to);
                            }
                        }

                        // Send to live WebSocket subscribers (ignore if no subscribers)
                        let _ = tx_clone.send(data);
                    }
                    Err(e) => {
                        debug!("PTY reader error for agent {}: {}", key_clone, e);
                        break;
                    }
                }
            }
        });

        // Spawn exit monitor task
        let key_for_exit = key.clone();
        let log_tx_for_exit = log_tx.clone();
        let svc_for_exit = service_name.clone();
        let proj_for_exit = project_name.clone();
        let exit_handle = tokio::task::spawn_blocking(move || {
            let mut child = child;
            let status = child.wait();
            match status {
                Ok(exit) => {
                    if exit.success() {
                        info!("Agent {} exited successfully", key_for_exit);
                        let _ = log_tx_for_exit.blocking_send(
                            format!("[{} EXIT] Agent exited successfully", svc_for_exit),
                        );
                    } else {
                        warn!("Agent {} exited with error: {:?}", key_for_exit, exit);
                        let _ = log_tx_for_exit.blocking_send(
                            format!("[{} EXIT] Agent exited with error", svc_for_exit),
                        );
                    }
                    let _ = log_tx_for_exit.blocking_send(
                        format!("[__STATUS__] project={} service={} status=stopped", proj_for_exit, svc_for_exit),
                    );
                }
                Err(e) => {
                    error!("Failed to wait for agent {}: {}", key_for_exit, e);
                    let _ = log_tx_for_exit.blocking_send(
                        format!("[__STATUS__] project={} service={} status=error", proj_for_exit, svc_for_exit),
                    );
                }
            }
        });

        self.agents.insert(key, ManagedAgent {
            output_tx,
            output_history,
            writer,
            master,
            _reader_handle: reader_handle,
            _exit_handle: exit_handle,
        });

        Ok(child_pid)
    }

    pub fn stop_agent(&mut self, key: &str) -> Result<()> {
        if let Some(agent) = self.agents.remove(key) {
            // Drop the writer and master to close the PTY, which will cause
            // the child process to receive SIGHUP
            drop(agent.writer);
            drop(agent.master);
            info!("Stopped agent {}", key);
            Ok(())
        } else {
            Err(crate::error::EnvibeError::Process(
                format!("Agent {} is not running", key),
            ))
        }
    }

    pub fn write_to_agent(&self, key: &str, data: &[u8]) -> Result<()> {
        if let Some(agent) = self.agents.get(key) {
            let mut writer = agent.writer.lock().map_err(|e| {
                crate::error::EnvibeError::Process(format!("Failed to lock PTY writer: {}", e))
            })?;
            writer.write_all(data).map_err(|e| {
                crate::error::EnvibeError::Process(format!("Failed to write to PTY: {}", e))
            })?;
            writer.flush().map_err(|e| {
                crate::error::EnvibeError::Process(format!("Failed to flush PTY: {}", e))
            })?;
            Ok(())
        } else {
            Err(crate::error::EnvibeError::Process(
                format!("Agent {} is not running", key),
            ))
        }
    }

    pub fn resize_agent(&self, key: &str, cols: u16, rows: u16) -> Result<()> {
        if let Some(agent) = self.agents.get(key) {
            let master = agent.master.lock().map_err(|e| {
                crate::error::EnvibeError::Process(format!("Failed to lock PTY master: {}", e))
            })?;
            master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| {
                    crate::error::EnvibeError::Process(format!("Failed to resize PTY: {}", e))
                })?;
            Ok(())
        } else {
            Err(crate::error::EnvibeError::Process(
                format!("Agent {} is not running", key),
            ))
        }
    }

    /// Subscribe to an agent's PTY output. Returns a broadcast receiver.
    pub fn subscribe(&self, key: &str) -> Option<broadcast::Receiver<Vec<u8>>> {
        self.agents.get(key).map(|a| a.output_tx.subscribe())
    }

    /// Get the buffered output history for an agent (for late-connecting clients).
    pub fn get_output_history(&self, key: &str) -> Option<Vec<u8>> {
        self.agents.get(key).and_then(|a| {
            a.output_history.lock().ok().map(|h| h.clone())
        })
    }

    pub fn is_running(&self, key: &str) -> bool {
        self.agents.contains_key(key)
    }
}
