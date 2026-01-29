import { useEffect, useRef } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { Terminal as TerminalIcon, Bot, Unplug } from 'lucide-react';
import type { TerminalType } from '../types';

interface UnifiedTerminalProps {
  projectName: string;
  terminalId: string;
  terminalName: string;
  terminalType: TerminalType;
  snapshotId?: string;
  isSelected?: boolean;
  onUserInput?: () => void; // Called when user types - used to clear waiting status
}

export function UnifiedTerminal({ projectName, terminalId, terminalName, terminalType, snapshotId, isSelected = true, onUserInput }: UnifiedTerminalProps) {
  const Icon = terminalType === 'agent' ? Bot : TerminalIcon;
  const label = terminalType === 'agent' ? 'agent' : 'shell';
  const terminalRef = useRef<HTMLDivElement>(null);
  const termInstanceRef = useRef<Terminal | null>(null);

  // Focus terminal when it becomes selected
  useEffect(() => {
    if (isSelected && termInstanceRef.current) {
      termInstanceRef.current.focus();
    }
  }, [isSelected]);


  useEffect(() => {
    if (!terminalRef.current) return;

    let cancelled = false;
    let ws: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Monaco, Menlo, monospace",
      theme: {
        background: '#0d1117',
        foreground: '#c9d1d9',
        cursor: '#58a6ff',
        selectionBackground: '#264f78',
        black: '#0d1117',
        red: '#ff7b72',
        green: '#3fb950',
        yellow: '#d29922',
        blue: '#58a6ff',
        magenta: '#bc8cff',
        cyan: '#39d2c0',
        white: '#c9d1d9',
        brightBlack: '#484f58',
        brightRed: '#ffa198',
        brightGreen: '#56d364',
        brightYellow: '#e3b341',
        brightBlue: '#79c0ff',
        brightMagenta: '#d2a8ff',
        brightCyan: '#56d4dd',
        brightWhite: '#f0f6fc',
      },
      allowProposedApi: true,
    });

    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(terminalRef.current);

    // Store ref for focus management
    termInstanceRef.current = term;

    requestAnimationFrame(() => {
      if (!cancelled) {
        fit.fit();
        // Auto-focus terminal so user can type immediately
        term.focus();
      }
    });

    term.onData((data) => {
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(data);
        // When user sends input, clear the waiting status
        if (onUserInput) {
          onUserInput();
        }
      }
    });

    term.onResize(({ cols, rows }) => {
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'resize', cols, rows }));
      }
    });

    const observer = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        if (!cancelled) fit.fit();
      });
    });
    observer.observe(terminalRef.current);

    const connect = async () => {
      if (cancelled) return;

      // Default fallback - actual URL comes from backend via IPC
      let wsUrl = 'ws://127.0.0.1:3848';
      try {
        if (window.envibe) {
          const baseUrl = await window.envibe.getBackendUrl();
          wsUrl = baseUrl.replace(/^http/, 'ws');
        }
      } catch {
        // use fallback
      }

      if (cancelled) return;

      // Build the WebSocket URL based on whether we have a snapshot
      let url: string;
      if (snapshotId) {
        url = `${wsUrl}/ws/shell/${encodeURIComponent(projectName)}/${encodeURIComponent(snapshotId)}/${encodeURIComponent(terminalId)}`;
      } else {
        url = `${wsUrl}/ws/shell/${encodeURIComponent(projectName)}/${encodeURIComponent(terminalId)}`;
      }

      const newWs = new WebSocket(url);
      newWs.binaryType = 'arraybuffer';
      ws = newWs;

      newWs.onopen = () => {
        if (cancelled) { newWs.close(); return; }
        const { cols, rows } = term;
        newWs.send(JSON.stringify({ type: 'resize', cols, rows }));
      };

      newWs.onmessage = (event) => {
        if (cancelled) return;
        if (event.data instanceof ArrayBuffer) {
          term.write(new Uint8Array(event.data));
        } else {
          term.write(event.data);
        }
      };

      newWs.onclose = () => {
        if (cancelled) return;
        reconnectTimer = setTimeout(() => {
          if (!cancelled) connect();
        }, 2000);
      };

      newWs.onerror = () => {};
    };

    connect();

    return () => {
      cancelled = true;
      observer.disconnect();
      if (reconnectTimer) clearTimeout(reconnectTimer);
      if (ws) {
        ws.close();
        ws = null;
      }
      termInstanceRef.current = null;
      term.dispose();
    };
  }, [projectName, terminalId, snapshotId, onUserInput]);

  return (
    <div className="panel h-full flex flex-col">
      <div className="panel-header">
        <span className="panel-title flex items-center gap-2">
          <Icon size={16} className="text-envibe-accent" />
          {terminalName}
          <span className="text-xs text-envibe-text-subtle">{label}</span>
        </span>
      </div>
      <div className="flex-1 min-h-0 p-1">
        <div ref={terminalRef} className="h-full w-full" />
      </div>
    </div>
  );
}

interface TerminalDisconnectedProps {
  terminalName?: string;
  terminalType?: TerminalType;
}

export function TerminalDisconnected({ terminalName = 'Terminal', terminalType = 'shell' }: TerminalDisconnectedProps) {
  const Icon = terminalType === 'agent' ? Bot : TerminalIcon;
  const label = terminalType === 'agent' ? 'agent' : 'shell';
  const message = terminalType === 'agent' ? 'Agent disconnected' : 'Terminal disconnected';

  return (
    <div className="panel h-full flex flex-col">
      <div className="panel-header">
        <span className="panel-title flex items-center gap-2">
          <Icon size={16} className="text-envibe-accent" />
          {terminalName}
          <span className="text-xs text-envibe-text-subtle">{label}</span>
        </span>
      </div>
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center text-envibe-text-muted">
          <Unplug size={32} className="mx-auto mb-2 opacity-50" />
          <p className="text-sm">{message}</p>
        </div>
      </div>
    </div>
  );
}

// Legacy exports for backwards compatibility
export const ShellTerminal = UnifiedTerminal;
export const ShellTerminalDisconnected = TerminalDisconnected;
