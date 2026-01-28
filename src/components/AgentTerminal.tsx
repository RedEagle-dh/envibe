import { useEffect, useRef } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { Bot, Unplug } from 'lucide-react';

interface AgentTerminalProps {
  projectName: string;
  serviceName: string;
}

export function AgentTerminal({ projectName, serviceName }: AgentTerminalProps) {
  const terminalRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!terminalRef.current) return;

    // Cancellation flag — prevents stale async operations from writing to the
    // terminal after cleanup (critical for React StrictMode double-mount).
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

    // Fit after a small delay to ensure DOM is ready
    requestAnimationFrame(() => {
      if (!cancelled) fit.fit();
    });

    // Forward user input to WebSocket
    term.onData((data) => {
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(data);
      }
    });

    // Forward resize events to backend
    term.onResize(({ cols, rows }) => {
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'resize', cols, rows }));
      }
    });

    // ResizeObserver to re-fit terminal
    const observer = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        if (!cancelled) fit.fit();
      });
    });
    observer.observe(terminalRef.current);

    // WebSocket connection
    const connect = async () => {
      if (cancelled) return;

      let wsUrl = 'ws://127.0.0.1:3847'; // fallback
      try {
        if (window.envibe) {
          const baseUrl = await window.envibe.getBackendUrl();
          wsUrl = baseUrl.replace(/^http/, 'ws');
        }
      } catch {
        // use fallback
      }

      if (cancelled) return;

      const url = `${wsUrl}/ws/terminal/${encodeURIComponent(projectName)}/${encodeURIComponent(serviceName)}`;
      const newWs = new WebSocket(url);
      newWs.binaryType = 'arraybuffer';
      ws = newWs;

      newWs.onopen = () => {
        if (cancelled) { newWs.close(); return; }
        // Send initial size
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
        // Attempt reconnect after 2 seconds
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
      term.dispose();
    };
  }, [projectName, serviceName]);

  return (
    <div className="panel h-full flex flex-col">
      <div className="panel-header">
        <span className="panel-title flex items-center gap-2">
          <Bot size={16} className="text-envibe-accent" />
          {serviceName}
          <span className="text-xs text-envibe-text-subtle">terminal</span>
        </span>
      </div>
      <div className="flex-1 min-h-0 p-1">
        <div ref={terminalRef} className="h-full w-full" />
      </div>
    </div>
  );
}

export function AgentTerminalDisconnected({ serviceName }: { serviceName: string }) {
  return (
    <div className="panel h-full flex flex-col">
      <div className="panel-header">
        <span className="panel-title flex items-center gap-2">
          <Bot size={16} className="text-envibe-accent" />
          {serviceName}
          <span className="text-xs text-envibe-text-subtle">terminal</span>
        </span>
      </div>
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center text-envibe-text-muted">
          <Unplug size={32} className="mx-auto mb-2 opacity-50" />
          <p className="text-sm">Start the agent to open the terminal</p>
        </div>
      </div>
    </div>
  );
}
