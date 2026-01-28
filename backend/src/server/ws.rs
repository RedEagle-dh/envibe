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
        let infos: Vec<ProjectInfo> = projects
            .iter()
            .map(|p| ProjectInfo::from_project(p, &app_state, &port_registry))
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

/// WebSocket handler for interactive agent terminal I/O.
/// Bridges bidirectional data between the xterm.js frontend and the PTY backend.
pub async fn ws_terminal_handler(
    socket: WebSocket,
    state: Arc<ServerState>,
    project: String,
    service: String,
) {
    let key = format!("{}:{}", project, service);
    info!("Terminal WebSocket connected for {}", key);

    // Get history and subscribe to live updates atomically (single lock)
    let (output_rx, history) = {
        let agent_manager = state.agent_manager.read().await;
        let rx = match agent_manager.subscribe(&key) {
            Some(rx) => rx,
            None => {
                error!("Agent {} is not running, cannot connect terminal", key);
                let (mut sender, _) = socket.split();
                let _ = sender.send(Message::Text(
                    format!("\r\nAgent '{}' is not running. Start it first.\r\n", service).into()
                )).await;
                let _ = sender.close().await;
                return;
            }
        };
        let history = agent_manager.get_output_history(&key).unwrap_or_default();
        (rx, history)
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Send buffered history first so the client sees everything from before connection
    if !history.is_empty() {
        debug!("Sending {} bytes of output history for {}", history.len(), key);
        if ws_sender.send(Message::Binary(history.into())).await.is_err() {
            debug!("Failed to send history for {}, aborting", key);
            return;
        }
    }

    // Sender task: forward live PTY output → WebSocket
    let key_for_sender = key.clone();
    let sender_task = tokio::spawn(async move {
        let mut rx = output_rx;
        loop {
            match rx.recv().await {
                Ok(data) => {
                    if ws_sender.send(Message::Binary(data.into())).await.is_err() {
                        debug!("WebSocket send failed for {}, closing sender", key_for_sender);
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    debug!("Terminal output lagged by {} messages for {}", n, key_for_sender);
                    // Continue receiving — just lost some output
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    debug!("Agent output channel closed for {}", key_for_sender);
                    // Send a final message indicating the agent has exited
                    let _ = ws_sender.send(Message::Text(
                        "\r\n[Agent process exited]\r\n".into()
                    )).await;
                    break;
                }
            }
        }
    });

    // Receiver task: forward WebSocket input → PTY
    let state_for_receiver = state.clone();
    let key_for_receiver = key.clone();
    let receiver_task = tokio::spawn(async move {
        while let Some(result) = ws_receiver.next().await {
            match result {
                Ok(Message::Text(text)) => {
                    // Check if it's a control message (JSON with "type" field)
                    if let Ok(control) = serde_json::from_str::<TerminalControl>(&text) {
                        if control.msg_type == "resize" {
                            if let (Some(cols), Some(rows)) = (control.cols, control.rows) {
                                let agent_manager = state_for_receiver.agent_manager.read().await;
                                if let Err(e) = agent_manager.resize_agent(&key_for_receiver, cols, rows) {
                                    debug!("Failed to resize agent {}: {}", key_for_receiver, e);
                                }
                            }
                        }
                    } else {
                        // Regular text input — send to PTY
                        let agent_manager = state_for_receiver.agent_manager.read().await;
                        if let Err(e) = agent_manager.write_to_agent(&key_for_receiver, text.as_bytes()) {
                            debug!("Failed to write to agent {}: {}", key_for_receiver, e);
                            break;
                        }
                    }
                }
                Ok(Message::Binary(data)) => {
                    // Binary data — send directly to PTY
                    let agent_manager = state_for_receiver.agent_manager.read().await;
                    if let Err(e) = agent_manager.write_to_agent(&key_for_receiver, &data) {
                        debug!("Failed to write binary to agent {}: {}", key_for_receiver, e);
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("Terminal WebSocket closed for {}", key_for_receiver);
                    break;
                }
                Err(e) => {
                    debug!("Terminal WebSocket error for {}: {}", key_for_receiver, e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = sender_task => {
            debug!("Sender task finished for {}", key);
        }
        _ = receiver_task => {
            debug!("Receiver task finished for {}", key);
        }
    }

    info!("Terminal WebSocket disconnected for {} (agent keeps running)", key);
}
