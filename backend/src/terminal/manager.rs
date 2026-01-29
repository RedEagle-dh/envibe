use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::error::{Error, Result};

/// Terminal session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalStatus {
    Connected,
    Disconnected,
}

/// Terminal session type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalType {
    Shell,
    Agent,
}

impl Default for TerminalType {
    fn default() -> Self {
        TerminalType::Shell
    }
}

/// A terminal session (metadata only, safe to share)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSession {
    pub id: String,
    pub name: String,
    pub status: TerminalStatus,
    /// Terminal type (shell or agent)
    #[serde(rename = "type", default)]
    pub session_type: TerminalType,
    /// Project this terminal belongs to
    #[serde(skip, default)]
    pub project_name: String,
    /// Snapshot ID if this terminal is in a snapshot context
    #[serde(skip)]
    pub snapshot_id: Option<String>,
}

/// Running terminal instance (contains non-Sync PTY handles)
struct RunningTerminal {
    #[allow(dead_code)]
    master: Box<dyn MasterPty + Send>,
    writer: Arc<StdMutex<Box<dyn Write + Send>>>,
    output_tx: broadcast::Sender<Vec<u8>>,
    output_history: Arc<StdMutex<Vec<u8>>>,
}

/// Session registry - just metadata, safe to share across threads
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TerminalSessions {
    pub sessions: HashMap<String, TerminalSession>,
    #[serde(skip, default)]
    shell_counter: u32,
    #[serde(skip, default)]
    agent_counter: u32,
}

impl TerminalSessions {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            shell_counter: 0,
            agent_counter: 0,
        }
    }

    pub async fn load(data_dir: &PathBuf) -> Result<Self> {
        let path = data_dir.join("terminals.json");
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(&path).await?;
        let mut sessions: TerminalSessions = serde_json::from_str(&content)?;

        // Mark all loaded sessions as disconnected since we're starting fresh
        for session in sessions.sessions.values_mut() {
            session.status = TerminalStatus::Disconnected;
        }

        Ok(sessions)
    }

    pub async fn save(&self, data_dir: &PathBuf) -> Result<()> {
        let path = data_dir.join("terminals.json");
        let content = serde_json::to_string_pretty(&self)?;
        fs::write(&path, content).await?;
        Ok(())
    }

    /// Get all terminal sessions for a project (optionally filtered by snapshot)
    pub fn get_sessions(&self, project_name: &str, snapshot_id: Option<&str>) -> Vec<TerminalSession> {
        self.sessions
            .values()
            .filter(|s| {
                s.project_name == project_name &&
                match snapshot_id {
                    Some(sid) => s.snapshot_id.as_deref() == Some(sid),
                    None => s.snapshot_id.is_none(),
                }
            })
            .cloned()
            .collect()
    }

    fn next_shell_counter(&mut self) -> u32 {
        self.shell_counter += 1;
        self.shell_counter
    }

    fn next_agent_counter(&mut self) -> u32 {
        self.agent_counter += 1;
        self.agent_counter
    }
}

/// Manages shell terminal PTY instances (contains non-Sync handles)
pub struct TerminalManager {
    /// Maps terminal ID -> running terminal
    terminals: HashMap<String, RunningTerminal>,
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            terminals: HashMap::new(),
        }
    }

    /// Create a new shell terminal session and start the PTY
    pub fn create_terminal(
        &mut self,
        sessions: &mut TerminalSessions,
        project_name: &str,
        working_dir: &PathBuf,
        snapshot_id: Option<String>,
    ) -> Result<TerminalSession> {
        let counter = sessions.next_shell_counter();
        let id = Uuid::new_v4().to_string();
        let name = format!("Terminal {}", counter);

        // Get default shell
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

        // Create PTY
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| Error::Process(format!("Failed to open PTY: {}", e)))?;

        // Build shell command
        let mut cmd = CommandBuilder::new(&shell);
        cmd.cwd(working_dir);

        // Set up environment
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        // Spawn the shell
        let _child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| Error::Process(format!("Failed to spawn shell: {}", e)))?;

        // Set up output reader
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| Error::Process(format!("Failed to clone reader: {}", e)))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| Error::Process(format!("Failed to take writer: {}", e)))?;

        // Create broadcast channel for output
        let (output_tx, _) = broadcast::channel::<Vec<u8>>(256);
        let output_history = Arc::new(StdMutex::new(Vec::new()));

        // Spawn output reader task
        let tx = output_tx.clone();
        let history = Arc::clone(&output_history);
        let terminal_id = id.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        // Store in history (cap at 100KB)
                        {
                            let mut hist = history.lock().unwrap();
                            hist.extend_from_slice(&data);
                            if hist.len() > 100_000 {
                                let drain_to = hist.len() - 100_000;
                                hist.drain(..drain_to);
                            }
                        }
                        // Broadcast to subscribers
                        let _ = tx.send(data);
                    }
                    Err(e) => {
                        tracing::debug!("Terminal {} read error: {}", terminal_id, e);
                        break;
                    }
                }
            }
            tracing::debug!("Terminal {} reader exited", terminal_id);
        });

        let session = TerminalSession {
            id: id.clone(),
            name,
            status: TerminalStatus::Connected,
            session_type: TerminalType::Shell,
            project_name: project_name.to_string(),
            snapshot_id,
        };

        let running = RunningTerminal {
            master: pair.master,
            writer: Arc::new(StdMutex::new(writer)),
            output_tx,
            output_history,
        };

        self.terminals.insert(id.clone(), running);
        sessions.sessions.insert(id.clone(), session.clone());

        tracing::info!("Created terminal {} for project {}", session.name, project_name);

        Ok(session)
    }

    /// Create a new agent terminal session and start the PTY with the specified agent command
    pub fn create_agent_terminal(
        &mut self,
        sessions: &mut TerminalSessions,
        project_name: &str,
        working_dir: &PathBuf,
        snapshot_id: Option<String>,
        command: &str,
    ) -> Result<TerminalSession> {
        let counter = sessions.next_agent_counter();
        let id = Uuid::new_v4().to_string();
        // Capitalize first letter for display name (claude -> Claude, codex -> Codex)
        let display_name = {
            let mut chars = command.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => command.to_string(),
            }
        };
        let name = format!("{} {}", display_name, counter);

        // Create PTY
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 30,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| Error::Process(format!("Failed to open PTY: {}", e)))?;

        // Build agent command
        let mut cmd = CommandBuilder::new(command);
        cmd.cwd(working_dir);

        // Set up environment
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        // Spawn the agent
        let _child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| Error::Process(format!("Failed to spawn {}: {}", command, e)))?;

        // Set up output reader
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| Error::Process(format!("Failed to clone reader: {}", e)))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| Error::Process(format!("Failed to take writer: {}", e)))?;

        // Create broadcast channel for output
        let (output_tx, _) = broadcast::channel::<Vec<u8>>(256);
        let output_history = Arc::new(StdMutex::new(Vec::new()));

        // Spawn output reader task
        let tx = output_tx.clone();
        let history = Arc::clone(&output_history);
        let terminal_id = id.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        // Store in history (cap at 128KB for agents)
                        {
                            let mut hist = history.lock().unwrap();
                            hist.extend_from_slice(&data);
                            if hist.len() > 128_000 {
                                let drain_to = hist.len() - 128_000;
                                hist.drain(..drain_to);
                            }
                        }
                        // Broadcast to subscribers
                        let _ = tx.send(data);
                    }
                    Err(e) => {
                        tracing::debug!("Agent terminal {} read error: {}", terminal_id, e);
                        break;
                    }
                }
            }
            tracing::debug!("Agent terminal {} reader exited", terminal_id);
        });

        let session = TerminalSession {
            id: id.clone(),
            name,
            status: TerminalStatus::Connected,
            session_type: TerminalType::Agent,
            project_name: project_name.to_string(),
            snapshot_id,
        };

        let running = RunningTerminal {
            master: pair.master,
            writer: Arc::new(StdMutex::new(writer)),
            output_tx,
            output_history,
        };

        self.terminals.insert(id.clone(), running);
        sessions.sessions.insert(id.clone(), session.clone());

        tracing::info!("Created agent terminal {} for project {}", session.name, project_name);

        Ok(session)
    }

    /// Close a terminal session
    pub fn close_terminal(&mut self, sessions: &mut TerminalSessions, terminal_id: &str) -> Result<()> {
        // Remove the running terminal (this drops the master, closing the PTY)
        self.terminals.remove(terminal_id);

        // Remove the session
        sessions.sessions.remove(terminal_id);

        tracing::info!("Closed terminal {}", terminal_id);

        Ok(())
    }

    /// Subscribe to terminal output
    pub fn subscribe(&self, terminal_id: &str) -> Option<broadcast::Receiver<Vec<u8>>> {
        self.terminals.get(terminal_id).map(|t| t.output_tx.subscribe())
    }

    /// Get output history for a terminal
    pub fn get_output_history(&self, terminal_id: &str) -> Option<Vec<u8>> {
        self.terminals
            .get(terminal_id)
            .map(|t| t.output_history.lock().unwrap().clone())
    }

    /// Write to terminal
    pub fn write_to_terminal(&self, terminal_id: &str, data: &[u8]) -> Result<()> {
        let terminal = self
            .terminals
            .get(terminal_id)
            .ok_or_else(|| Error::NotFound(format!("Terminal {} not found", terminal_id)))?;

        let mut writer = terminal.writer.lock().unwrap();
        writer
            .write_all(data)
            .map_err(|e| Error::Process(format!("Failed to write to terminal: {}", e)))?;
        writer
            .flush()
            .map_err(|e| Error::Process(format!("Failed to flush terminal: {}", e)))?;

        Ok(())
    }

    /// Resize terminal
    pub fn resize_terminal(&self, terminal_id: &str, cols: u16, rows: u16) -> Result<()> {
        let terminal = self
            .terminals
            .get(terminal_id)
            .ok_or_else(|| Error::NotFound(format!("Terminal {} not found", terminal_id)))?;

        terminal
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| Error::Process(format!("Failed to resize terminal: {}", e)))?;

        Ok(())
    }

    /// Check if terminal is running
    pub fn is_running(&self, terminal_id: &str) -> bool {
        self.terminals.contains_key(terminal_id)
    }
}
