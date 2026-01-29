use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use super::api::{ProjectInfo, ServerState};

/// Control message sent from the frontend to resize the terminal
#[derive(Debug, Deserialize)]
struct TerminalControl {
    #[serde(rename = "type")]
    msg_type: String,
    cols: Option<u16>,
    rows: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    // Client -> Server
    Subscribe { project: Option<String>, service: Option<String> },
    Unsubscribe,
    StartService { project: String, service: String },
    StopService { project: String, service: String },
    RestartService { project: String, service: String },

    // Server -> Client
    Log { timestamp: String, level: String, service: Option<String>, project: Option<String>, message: String },
    ServiceUpdate { project: String, service: String, status: String, port: Option<u16> },
    ProjectsUpdate { projects: Vec<ProjectInfo> },
    Error { message: String },
}

pub async fn handle_websocket(socket: WebSocket, state: Arc<ServerState>) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<WsMessage>(100);

    // Spawn task to forward messages to the websocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Send initial projects list
    {
        let projects = state.projects.read().await;
        let app_state = state.state.read().await;
        let port_registry = state.port_registry.read().await;
        let worktree_manager = state.worktree_manager.read().await;
        let terminal_sessions = state.terminal_sessions.read().await;
        let infos: Vec<ProjectInfo> = projects
            .iter()
            .map(|p| ProjectInfo::from_project(p, &app_state, &port_registry, &worktree_manager, &terminal_sessions))
            .collect();
        let _ = tx.send(WsMessage::ProjectsUpdate {
            projects: infos,
        }).await;
    }

    // Handle incoming messages
    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                if let Ok(msg) = serde_json::from_str::<WsMessage>(&text) {
                    match msg {
                        WsMessage::Subscribe { project, service } => {
                            tracing::info!("Client subscribed to {:?}/{:?}", project, service);
                        }
                        WsMessage::StartService { project, service } => {
                            tracing::info!("Starting {}/{}", project, service);
                        }
                        WsMessage::StopService { project, service } => {
                            tracing::info!("Stopping {}/{}", project, service);
                        }
                        WsMessage::RestartService { project, service } => {
                            tracing::info!("Restarting {}/{}", project, service);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    send_task.abort();
}

/// WebSocket handler for shell/agent terminal I/O.
/// Bridges bidirectional data between the xterm.js frontend and the shell PTY.
pub async fn ws_shell_handler(
    socket: WebSocket,
    state: Arc<ServerState>,
    _project: String,
    terminal_id: String,
) {
    info!("Shell WebSocket connected for terminal {}", terminal_id);

    // Check if terminal exists first (quick check, drop guard immediately)
    let exists = {
        let terminal_manager = state.terminal_manager.lock().unwrap();
        terminal_manager.is_running(&terminal_id)
    };

    if !exists {
        error!("Terminal {} not found or not running", terminal_id);
        let (mut sender, _) = socket.split();
        let _ = sender.send(Message::Text(
            format!("\r\nTerminal '{}' not found. Create it first.\r\n", terminal_id).into()
        )).await;
        let _ = sender.close().await;
        return;
    }

    // Now get history and subscribe (terminal exists, so these won't fail)
    let (output_rx, history) = {
        let terminal_manager = state.terminal_manager.lock().unwrap();
        let rx = terminal_manager.subscribe(&terminal_id).unwrap();
        let history = terminal_manager.get_output_history(&terminal_id).unwrap_or_default();
        (rx, history)
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Send buffered history first
    if !history.is_empty() {
        debug!("Sending {} bytes of output history for terminal {}", history.len(), terminal_id);
        if ws_sender.send(Message::Binary(history.into())).await.is_err() {
            debug!("Failed to send history for terminal {}, aborting", terminal_id);
            return;
        }
    }

    // Sender task: forward PTY output → WebSocket
    let terminal_id_for_sender = terminal_id.clone();
    let sender_task = tokio::spawn(async move {
        let mut rx = output_rx;
        loop {
            match rx.recv().await {
                Ok(data) => {
                    if ws_sender.send(Message::Binary(data.into())).await.is_err() {
                        debug!("WebSocket send failed for terminal {}, closing sender", terminal_id_for_sender);
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    debug!("Terminal output lagged by {} messages for {}", n, terminal_id_for_sender);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    debug!("Terminal output channel closed for {}", terminal_id_for_sender);
                    let _ = ws_sender.send(Message::Text(
                        "\r\n[Terminal closed]\r\n".into()
                    )).await;
                    break;
                }
            }
        }
    });

    // Receiver task: forward WebSocket input → PTY
    let state_for_receiver = state.clone();
    let terminal_id_for_receiver = terminal_id.clone();
    let receiver_task = tokio::spawn(async move {
        while let Some(result) = ws_receiver.next().await {
            match result {
                Ok(Message::Text(text)) => {
                    // Check if it's a control message (JSON with "type" field)
                    if let Ok(control) = serde_json::from_str::<TerminalControl>(&text) {
                        if control.msg_type == "resize" {
                            if let (Some(cols), Some(rows)) = (control.cols, control.rows) {
                                let terminal_manager = state_for_receiver.terminal_manager.lock().unwrap();
                                if let Err(e) = terminal_manager.resize_terminal(&terminal_id_for_receiver, cols, rows) {
                                    debug!("Failed to resize terminal {}: {}", terminal_id_for_receiver, e);
                                }
                            }
                        }
                    } else {
                        // Regular text input — send to PTY
                        let terminal_manager = state_for_receiver.terminal_manager.lock().unwrap();
                        if let Err(e) = terminal_manager.write_to_terminal(&terminal_id_for_receiver, text.as_bytes()) {
                            debug!("Failed to write to terminal {}: {}", terminal_id_for_receiver, e);
                            break;
                        }
                    }
                }
                Ok(Message::Binary(data)) => {
                    let terminal_manager = state_for_receiver.terminal_manager.lock().unwrap();
                    if let Err(e) = terminal_manager.write_to_terminal(&terminal_id_for_receiver, &data) {
                        debug!("Failed to write binary to terminal {}: {}", terminal_id_for_receiver, e);
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("Shell WebSocket closed for terminal {}", terminal_id_for_receiver);
                    break;
                }
                Err(e) => {
                    debug!("Shell WebSocket error for terminal {}: {}", terminal_id_for_receiver, e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = sender_task => {
            debug!("Sender task finished for terminal {}", terminal_id);
        }
        _ = receiver_task => {
            debug!("Receiver task finished for terminal {}", terminal_id);
        }
    }

    info!("Shell WebSocket disconnected for terminal {} (shell keeps running)", terminal_id);
}
