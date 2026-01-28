import {
  KeyRound,
  Play,
  Square,
} from 'lucide-react';
import { useStore, useSelectedProject } from '../stores/useStore';

export function Sidebar() {
  const showEnvPanel = useStore((s) => s.showEnvPanel);
  const setShowEnvPanel = useStore((s) => s.setShowEnvPanel);
  const refreshProjects = useStore((s) => s.refreshProjects);
  const selectedProject = useSelectedProject();

  const stoppedServices = selectedProject?.services.filter(
    (s) => s.status === 'stopped' || s.status === 'error'
  ) ?? [];
  const runningServices = selectedProject?.services.filter(
    (s) => s.status === 'running'
  ) ?? [];

  const handleStartAll = async () => {
    if (!selectedProject || !window.envibe || stoppedServices.length === 0) return;
    for (const service of stoppedServices) {
      await window.envibe.startService(selectedProject.name, service.name);
    }
    setTimeout(() => refreshProjects(), 500);
  };

  const handleStopAll = async () => {
    if (!selectedProject || !window.envibe || runningServices.length === 0) return;
    for (const service of runningServices) {
      await window.envibe.stopService(selectedProject.name, service.name);
    }
    setTimeout(() => refreshProjects(), 500);
  };

  return (
    <aside className="w-14 bg-envibe-bg-secondary border-r border-envibe-border flex flex-col items-center py-4 gap-2">
      <NavButton
        icon={KeyRound}
        label="Environment"
        active={showEnvPanel}
        onClick={() => setShowEnvPanel(!showEnvPanel)}
      />

      <div className="flex-1" />

      <div className="flex flex-col gap-1">
        <QuickAction
          icon={Play}
          label="Start All"
          color="success"
          disabled={stoppedServices.length === 0}
          onClick={handleStartAll}
        />
        <QuickAction
          icon={Square}
          label="Stop All"
          color="danger"
          disabled={runningServices.length === 0}
          onClick={handleStopAll}
        />
      </div>
    </aside>
  );
}

interface NavButtonProps {
  icon: React.ComponentType<{ size?: number }>;
  label: string;
  active?: boolean;
  onClick: () => void;
}

function NavButton({ icon: Icon, label, active, onClick }: NavButtonProps) {
  return (
    <button
      className={`
        relative w-10 h-10 rounded-lg flex items-center justify-center transition-colors
        ${active
          ? 'bg-envibe-accent/20 text-envibe-accent'
          : 'text-envibe-text-muted hover:text-envibe-text hover:bg-envibe-bg-tertiary'
        }
      `}
      onClick={onClick}
      title={label}
    >
      <Icon size={20} />
    </button>
  );
}

interface QuickActionProps {
  icon: React.ComponentType<{ size?: number }>;
  label: string;
  color: 'success' | 'danger';
  disabled?: boolean;
  onClick: () => void;
}

function QuickAction({ icon: Icon, label, color, disabled, onClick }: QuickActionProps) {
  const colorClasses = {
    success: 'text-envibe-success hover:bg-envibe-success/20',
    danger: 'text-envibe-danger hover:bg-envibe-danger/20',
  };

  return (
    <button
      className={`w-10 h-10 rounded-lg flex items-center justify-center transition-colors ${
        disabled
          ? 'text-envibe-text-muted/30 cursor-not-allowed'
          : colorClasses[color]
      }`}
      onClick={disabled ? undefined : onClick}
      disabled={disabled}
      title={label}
    >
      <Icon size={18} />
    </button>
  );
}
