import { X, Terminal, Bot, ScrollText } from 'lucide-react';
import { useStore, useContextTerminals } from '../stores/useStore';
import type { TerminalSession } from '../types';

function getTerminalIcon(terminal: TerminalSession) {
  return terminal.type === 'agent' ? Bot : Terminal;
}

export function TerminalTabs() {
  const openTerminals = useStore((s) => s.openTerminals);
  const selectedTerminalId = useStore((s) => s.selectedTerminalId);
  const selectTerminal = useStore((s) => s.selectTerminal);
  const closeTerminalTab = useStore((s) => s.closeTerminalTab);
  const closeTerminal = useStore((s) => s.closeTerminal);
  const contextTerminals = useContextTerminals();

  // Only show terminals that are actually open in tabs
  const visibleTerminals = contextTerminals.filter((t) => openTerminals.includes(t.id));

  // Show the tab bar if there are any terminals (we always show Logs tab when terminals exist)
  if (visibleTerminals.length === 0) {
    return null;
  }

  const handleCloseTab = async (e: React.MouseEvent, terminalId: string) => {
    e.stopPropagation();
    // Close the tab locally first for immediate feedback
    closeTerminalTab(terminalId);
    // Then close on the backend
    await closeTerminal(terminalId);
  };

  const handleShowLogs = () => {
    // Deselect terminal to show logs
    selectTerminal(null);
  };

  const isLogsSelected = selectedTerminalId === null;

  return (
    <div className="flex items-center gap-1 px-2 py-1 bg-envibe-bg-secondary border-b border-envibe-border overflow-x-auto">
      {/* Logs tab - always first */}
      <button
        onClick={handleShowLogs}
        className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm transition-colors ${
          isLogsSelected
            ? 'bg-envibe-bg-tertiary text-envibe-text'
            : 'text-envibe-text-muted hover:text-envibe-text hover:bg-envibe-bg-tertiary/50'
        }`}
      >
        <ScrollText size={14} className="text-envibe-accent" />
        <span>Logs</span>
      </button>

      {/* Separator */}
      <div className="w-px h-4 bg-envibe-border mx-1" />

      {/* Terminal tabs */}
      {visibleTerminals.map((terminal) => {
        const Icon = getTerminalIcon(terminal);
        return (
          <button
            key={terminal.id}
            onClick={() => selectTerminal(terminal.id)}
            className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm transition-colors group ${
              selectedTerminalId === terminal.id
                ? 'bg-envibe-bg-tertiary text-envibe-text'
                : 'text-envibe-text-muted hover:text-envibe-text hover:bg-envibe-bg-tertiary/50'
            }`}
          >
            <Icon size={14} className={terminal.status === 'connected' ? 'text-envibe-success' : 'text-envibe-text-muted'} />
            <span className="truncate max-w-[120px]">{terminal.name}</span>
            <button
              onClick={(e) => handleCloseTab(e, terminal.id)}
              className="p-0.5 rounded hover:bg-envibe-danger/20 hover:text-envibe-danger opacity-0 group-hover:opacity-100 transition-all"
              title="Close terminal"
            >
              <X size={12} />
            </button>
          </button>
        );
      })}
    </div>
  );
}
