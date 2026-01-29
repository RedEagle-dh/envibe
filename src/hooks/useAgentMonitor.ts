import { useEffect, useRef } from 'react';
import { useStore } from '../stores/useStore';
import type { AgentStatus, TerminalSession } from '../types';

// BEL character - used by Claude Code/Codex to signal they need user attention
const BEL_BYTE = 0x07;

// Check if data contains meaningful content (not just escape sequences/control chars)
function hasRealContent(data: ArrayBuffer | string): boolean {
  let text: string;
  if (data instanceof ArrayBuffer) {
    const bytes = new Uint8Array(data);
    text = new TextDecoder().decode(bytes);
  } else {
    text = data;
  }

  // Remove ANSI escape sequences
  const withoutAnsi = text.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '');
  // Remove other control characters (except newline/tab)
  const withoutControl = withoutAnsi.replace(/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/g, '');
  // Check if there's any printable content left (more than just whitespace)
  return withoutControl.trim().length > 0;
}

// Time to wait with no output before determining agent state
const IDLE_DELAY_MS = 3000;

interface AgentConnection {
  ws: WebSocket;
  terminalId: string;
  projectName: string;
  snapshotId?: string;
  idleTimer?: ReturnType<typeof setTimeout>;      // Timer for silence detection
  lastBelTime?: number;                           // Timestamp of last BEL
  lastOutputTime?: number;                        // Timestamp of last non-BEL output
}

/**
 * Background monitor for all agent terminals across all projects.
 * Maintains WebSocket connections to detect BEL (bell) characters
 * that signal when an agent needs user attention.
 *
 * This runs independently of which project/terminal is currently selected.
 */
export function useAgentMonitor() {
  const projects = useStore((s) => s.projects);
  const openTerminals = useStore((s) => s.openTerminals);
  const updateTerminalAgentStatus = useStore((s) => s.updateTerminalAgentStatus);

  // Track active connections
  const connectionsRef = useRef<Map<string, AgentConnection>>(new Map());
  // Track current status to avoid redundant updates
  const statusRef = useRef<Map<string, AgentStatus>>(new Map());

  useEffect(() => {
    // Collect all open agent terminals across all projects
    const agentTerminals: { terminal: TerminalSession; projectName: string; snapshotId?: string }[] = [];

    for (const project of projects) {
      // Project-level terminals
      for (const terminal of project.terminals) {
        if (terminal.type === 'agent' && openTerminals.includes(terminal.id)) {
          agentTerminals.push({ terminal, projectName: project.name });
        }
      }
      // Snapshot-level terminals
      for (const snapshot of project.snapshots ?? []) {
        for (const terminal of snapshot.terminals) {
          if (terminal.type === 'agent' && openTerminals.includes(terminal.id)) {
            agentTerminals.push({ terminal, projectName: project.name, snapshotId: snapshot.id });
          }
        }
      }
    }

    const currentIds = new Set(agentTerminals.map((t) => t.terminal.id));
    const existingIds = new Set(connectionsRef.current.keys());

    // Close connections for terminals that are no longer open
    for (const id of existingIds) {
      if (!currentIds.has(id)) {
        const conn = connectionsRef.current.get(id);
        if (conn) {
          if (conn.idleTimer) clearTimeout(conn.idleTimer);
          conn.ws.close();
          connectionsRef.current.delete(id);
          statusRef.current.delete(id);
        }
      }
    }

    // Create connections for new terminals
    for (const { terminal, projectName, snapshotId } of agentTerminals) {
      if (!connectionsRef.current.has(terminal.id)) {
        createConnection(terminal.id, projectName, snapshotId);
      }
    }

    async function createConnection(terminalId: string, projectName: string, snapshotId?: string) {
      // Get backend URL
      let wsUrl = 'ws://127.0.0.1:3848';
      try {
        if (window.envibe) {
          const baseUrl = await window.envibe.getBackendUrl();
          wsUrl = baseUrl.replace(/^http/, 'ws');
        }
      } catch {
        // use fallback
      }

      // Build WebSocket URL
      let url: string;
      if (snapshotId) {
        url = `${wsUrl}/ws/shell/${encodeURIComponent(projectName)}/${encodeURIComponent(snapshotId)}/${encodeURIComponent(terminalId)}`;
      } else {
        url = `${wsUrl}/ws/shell/${encodeURIComponent(projectName)}/${encodeURIComponent(terminalId)}`;
      }

      const ws = new WebSocket(url);
      ws.binaryType = 'arraybuffer';

      const connection: AgentConnection = { ws, terminalId, projectName, snapshotId };
      connectionsRef.current.set(terminalId, connection);
      statusRef.current.set(terminalId, 'idle');

      ws.onmessage = (event) => {
        const conn = connectionsRef.current.get(terminalId);
        if (!conn) return;

        const now = Date.now();

        // Check for BEL character
        let hasBel = false;
        if (event.data instanceof ArrayBuffer) {
          const bytes = new Uint8Array(event.data);
          hasBel = bytes.includes(BEL_BYTE);
        } else if (typeof event.data === 'string') {
          hasBel = event.data.includes('\x07');
        }

        // Track timestamps
        if (hasBel) {
          conn.lastBelTime = now;
        }
        // Only count as "real output" if it has meaningful content
        // (not just escape sequences or control characters)
        if (!hasBel && hasRealContent(event.data)) {
          conn.lastOutputTime = now;
        }

        // Clear idle timer on any output - we'll restart it
        if (conn.idleTimer) {
          clearTimeout(conn.idleTimer);
          conn.idleTimer = undefined;
        }

        // If we got new output while in waiting_input or completed, go back to busy
        const currentStatus = statusRef.current.get(terminalId);
        if (currentStatus === 'waiting_input' || currentStatus === 'completed') {
          statusRef.current.set(terminalId, 'busy');
          updateTerminalAgentStatus(terminalId, 'busy');
        } else if (currentStatus !== 'busy') {
          statusRef.current.set(terminalId, 'busy');
          updateTerminalAgentStatus(terminalId, 'busy');
        }

        // Start idle timer - when it fires, determine waiting_input vs completed
        conn.idleTimer = setTimeout(() => {
          const status = statusRef.current.get(terminalId);
          if (status !== 'busy') return; // Already in a final state

          const lastBel = conn.lastBelTime ?? 0;
          const lastOutput = conn.lastOutputTime ?? 0;

          // If BEL was more recent than non-BEL output, agent is waiting for input
          // (BEL then silence = waiting for user)
          if (lastBel > lastOutput) {
            statusRef.current.set(terminalId, 'waiting_input');
            updateTerminalAgentStatus(terminalId, 'waiting_input');
          } else {
            // Non-BEL output was more recent, agent finished and is idle
            statusRef.current.set(terminalId, 'completed');
            updateTerminalAgentStatus(terminalId, 'completed');
          }
        }, IDLE_DELAY_MS);
      };

      ws.onclose = () => {
        // Don't remove from map - let the effect handle reconnection if needed
      };

      ws.onerror = () => {
        // Errors are handled by onclose
      };
    }

    // Cleanup on unmount
    return () => {
      // Note: We don't close connections here because this effect re-runs
      // when projects/openTerminals change. Connections are managed above.
    };
  }, [projects, openTerminals, updateTerminalAgentStatus]);

  // Cleanup all connections on unmount
  useEffect(() => {
    return () => {
      for (const conn of connectionsRef.current.values()) {
        if (conn.idleTimer) clearTimeout(conn.idleTimer);
        conn.ws.close();
      }
      connectionsRef.current.clear();
      statusRef.current.clear();
    };
  }, []);

  // Function to clear waiting/completed status when user interacts with a terminal
  const clearWaitingStatus = (terminalId: string) => {
    const currentStatus = statusRef.current.get(terminalId);
    if (currentStatus === 'waiting_input' || currentStatus === 'completed') {
      statusRef.current.set(terminalId, 'busy');
      updateTerminalAgentStatus(terminalId, 'busy');
    }
  };

  return { clearWaitingStatus };
}
