import { Settings, HelpCircle, RefreshCw } from 'lucide-react';
import { useStore } from '../stores/useStore';

export function Header() {
  const setSettingsModalOpen = useStore((s) => s.setSettingsModalOpen);
  const refreshProjects = useStore((s) => s.refreshProjects);

  return (
    <header className="h-12 bg-envibe-bg-secondary border-b border-envibe-border flex items-center px-4 drag-region">
      {/* Spacer for macOS traffic lights */}
      <div className="w-20 flex-shrink-0" />

      <div className="flex-1 flex items-center justify-center no-drag">
        <h1 className="text-sm font-semibold text-envibe-text flex items-center gap-2">
          <span className="text-envibe-accent">●</span>
          Envibe
          <span className="text-envibe-text-subtle text-xs font-normal ml-2">
            Dev Orchestration
          </span>
        </h1>
      </div>

      <div className="flex items-center gap-1 no-drag">
        <button className="icon-btn" title="Refresh projects" onClick={refreshProjects}>
          <RefreshCw size={16} />
        </button>
        <button className="icon-btn" title="Help">
          <HelpCircle size={16} />
        </button>
        <button className="icon-btn" title="Settings" onClick={() => setSettingsModalOpen(true)}>
          <Settings size={16} />
        </button>
      </div>
    </header>
  );
}
