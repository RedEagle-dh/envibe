import { useState, useEffect, useRef } from 'react';
import { Play, Square, RotateCw, Layers, Database, Server, Terminal as TerminalIcon, Bot, Edit2, Check, X, Plus, ChevronDown } from 'lucide-react';
import { useStore, useSelectedProject, useSelectedSnapshot, useContextServices } from '../stores/useStore';
import type { Service, ServiceStatus } from '../types';

type ServiceTab = 'processes' | 'docker';

export function ServicesPanel() {
  const selectedProject = useSelectedProject();
  const selectedSnapshot = useSelectedSnapshot();
  const selectedServiceName = useStore((s) => s.selectedServiceName);
  const selectService = useStore((s) => s.selectService);
  const createTerminal = useStore((s) => s.createTerminal);
  const selectedSnapshotId = useStore((s) => s.selectedSnapshotId);
  const [activeTab, setActiveTab] = useState<ServiceTab>('processes');
  const [showDropdown, setShowDropdown] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Use context-aware services (from snapshot if selected, otherwise from project)
  const contextServices = useContextServices();

  const processServices = contextServices.filter((s) => s.type === 'process');
  const dockerServices = contextServices.filter((s) => s.type === 'docker' || s.type === 'compose');
  const filteredServices = activeTab === 'processes'
    ? processServices
    : dockerServices;

  // Auto-select tab that has services when project/snapshot changes
  useEffect(() => {
    if (contextServices.length > 0) {
      const hasProcesses = contextServices.some((s) => s.type === 'process');
      const hasDocker = contextServices.some((s) => s.type === 'docker' || s.type === 'compose');
      if (hasProcesses) {
        setActiveTab('processes');
      } else if (hasDocker) {
        setActiveTab('docker');
      } else {
        setActiveTab('processes');
      }
    }
  }, [selectedProject?.name, selectedSnapshotId, contextServices.length]);

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setShowDropdown(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleNewTerminal = async (terminalType: 'shell' | 'agent', agentCommand?: string) => {
    setShowDropdown(false);
    if (selectedProject) {
      await createTerminal(selectedProject.name, selectedSnapshotId ?? undefined, terminalType, agentCommand);
    }
  };

  if (!selectedProject) {
    return (
      <div className="panel h-full flex flex-col">
        <div className="panel-header">
          <span className="panel-title flex items-center gap-2">
            <Layers size={16} className="text-envibe-accent" />
            Services
          </span>
        </div>
        <div className="panel-content flex-1 flex items-center justify-center">
          <div className="text-center text-envibe-text-muted p-4">
            <Layers size={32} className="mx-auto mb-2 opacity-50" />
            <p className="text-sm">Select a project</p>
          </div>
        </div>
      </div>
    );
  }

  const contextLabel = selectedSnapshot ? selectedSnapshot.name : selectedProject.name;
  const runningCount = contextServices.filter((s) => s.status === 'running').length;

  return (
    <div className="panel h-full flex flex-col">
      <div className="flex flex-col border-b border-envibe-border">
        <div className="flex items-center justify-between px-4 py-3">
          <div className="flex items-center gap-2 min-w-0">
            <Layers size={16} className="text-envibe-accent flex-shrink-0" />
            <span className="panel-title truncate">{contextLabel}</span>
            {selectedSnapshot && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-envibe-accent/20 text-envibe-accent flex-shrink-0">
                snapshot
              </span>
            )}
          </div>
          <div className="flex items-center gap-2 flex-shrink-0">
            <span className="text-xs text-envibe-text-subtle">
              {runningCount}/{contextServices.length} running
            </span>
            <div className="relative" ref={dropdownRef}>
              <button
                onClick={() => setShowDropdown(!showDropdown)}
                className="flex items-center gap-0.5 p-1 rounded text-envibe-text-muted hover:text-envibe-accent hover:bg-envibe-accent/20 transition-colors"
                title="New terminal or agent"
              >
                <Plus size={14} />
                <ChevronDown size={10} />
              </button>
              {showDropdown && (
                <div className="absolute right-0 top-full mt-1 z-50 min-w-[140px] bg-envibe-bg-secondary border border-envibe-border rounded-md shadow-lg py-1">
                  <button
                    onClick={() => handleNewTerminal('shell')}
                    className="w-full flex items-center gap-2 px-3 py-2 text-sm text-envibe-text hover:bg-envibe-bg-tertiary transition-colors"
                  >
                    <TerminalIcon size={14} />
                    Terminal
                  </button>
                  <div className="border-t border-envibe-border my-1" />
                  <button
                    onClick={() => handleNewTerminal('agent', 'claude')}
                    className="w-full flex items-center gap-2 px-3 py-2 text-sm text-envibe-text hover:bg-envibe-bg-tertiary transition-colors"
                  >
                    <Bot size={14} />
                    Claude
                  </button>
                  <button
                    onClick={() => handleNewTerminal('agent', 'codex')}
                    className="w-full flex items-center gap-2 px-3 py-2 text-sm text-envibe-text hover:bg-envibe-bg-tertiary transition-colors"
                  >
                    <Bot size={14} />
                    Codex
                  </button>
                </div>
              )}
            </div>
          </div>
        </div>
        <div className="flex border-t border-envibe-border">
          <button
            className={`flex-1 px-4 py-2 text-xs font-medium transition-colors ${
              activeTab === 'processes'
                ? 'text-envibe-accent border-b-2 border-envibe-accent bg-envibe-bg-tertiary/50'
                : 'text-envibe-text-muted hover:text-envibe-text'
            }`}
            onClick={() => setActiveTab('processes')}
          >
            Processes
            <span className="ml-1.5 text-envibe-text-subtle">{processServices.length}</span>
          </button>
          <button
            className={`flex-1 px-4 py-2 text-xs font-medium transition-colors ${
              activeTab === 'docker'
                ? 'text-envibe-accent border-b-2 border-envibe-accent bg-envibe-bg-tertiary/50'
                : 'text-envibe-text-muted hover:text-envibe-text'
            }`}
            onClick={() => setActiveTab('docker')}
          >
            Docker
            <span className="ml-1.5 text-envibe-text-subtle">{dockerServices.length}</span>
          </button>
        </div>
      </div>
      <div className="panel-content flex-1 overflow-y-auto">
        {filteredServices.length === 0 ? (
          <div className="p-4 text-center text-envibe-text-muted">
            <p className="text-sm">
              No {activeTab === 'processes' ? 'process' : 'Docker'} services
            </p>
          </div>
        ) : (
          <ul>
            {filteredServices.map((service) => (
              <ServiceItem
                key={service.name}
                service={service}
                projectName={selectedProject.name}
                selected={service.name === selectedServiceName}
                onClick={() => selectService(service.name)}
              />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

interface ServiceItemProps {
  service: Service;
  projectName: string;
  selected: boolean;
  onClick: () => void;
}

function ServiceItem({ service, projectName, selected, onClick }: ServiceItemProps) {
  const refreshProjects = useStore((s) => s.refreshProjects);
  const [editingPort, setEditingPort] = useState(false);
  const [portValue, setPortValue] = useState(service.port?.toString() || '');
  const [portError, setPortError] = useState<string | null>(null);

  const isDockerService = service.type === 'docker' || service.type === 'compose';
  const canEditPort = isDockerService && (service.status === 'stopped' || service.status === 'error');

  const handleStart = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (window.envibe) {
      await window.envibe.startService(projectName, service.name);
      setTimeout(() => refreshProjects(), 500);
    }
  };

  const handleStop = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (window.envibe) {
      await window.envibe.stopService(projectName, service.name);
      setTimeout(() => refreshProjects(), 500);
    }
  };

  const handleRestart = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (window.envibe) {
      await window.envibe.restartService(projectName, service.name);
      setTimeout(() => refreshProjects(), 1000);
    }
  };

  const handleEditPort = (e: React.MouseEvent) => {
    e.stopPropagation();
    setPortValue(service.port?.toString() || service.internalPort?.toString() || '');
    setPortError(null);
    setEditingPort(true);
  };

  const handleSavePort = async (e: React.MouseEvent) => {
    e.stopPropagation();
    const port = parseInt(portValue, 10);
    if (isNaN(port) || port < 1024 || port > 65535) {
      setPortError('Port must be 1024-65535');
      return;
    }

    if (window.envibe) {
      try {
        const result = await window.envibe.setServicePort(projectName, service.name, port);
        if (result?.status === 'ok') {
          setEditingPort(false);
          setPortError(null);
          refreshProjects();
        } else {
          setPortError('Port in use');
        }
      } catch {
        setPortError('Failed to set port');
      }
    }
  };

  const handleCancelEdit = (e: React.MouseEvent) => {
    e.stopPropagation();
    setEditingPort(false);
    setPortError(null);
  };

  const TypeIcon = getTypeIcon(service.type);
  const statusClass = getStatusClass(service.status);
  const statusText = getStatusText(service.status);

  return (
    <li
      className={`list-item ${selected ? 'selected' : ''}`}
      onClick={onClick}
    >
      <div className="flex items-start gap-3">
        <div className={`status-dot mt-1.5 ${statusClass}`} />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <TypeIcon size={14} className="text-envibe-text-muted flex-shrink-0" />
            <span className="font-medium text-sm truncate">{service.name}</span>
            <span className={`badge ${getBadgeClass(service.status)}`}>
              {statusText}
            </span>
          </div>

          <div className="flex items-center gap-2 mt-1 text-xs text-envibe-text-muted">
            {isDockerService && (
              editingPort ? (
                <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
                  <span className="text-envibe-text-subtle">:</span>
                  <input
                    type="number"
                    value={portValue}
                    onChange={(e) => setPortValue(e.target.value)}
                    className="w-16 bg-envibe-bg-tertiary border border-envibe-border rounded px-1 py-0.5 text-envibe-accent focus:outline-none focus:border-envibe-accent"
                    min={1024}
                    max={65535}
                    autoFocus
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') handleSavePort(e as unknown as React.MouseEvent);
                      if (e.key === 'Escape') handleCancelEdit(e as unknown as React.MouseEvent);
                    }}
                  />
                  <button
                    className="p-0.5 text-envibe-success hover:bg-envibe-success/20 rounded"
                    onClick={handleSavePort}
                    title="Save"
                  >
                    <Check size={12} />
                  </button>
                  <button
                    className="p-0.5 text-envibe-danger hover:bg-envibe-danger/20 rounded"
                    onClick={handleCancelEdit}
                    title="Cancel"
                  >
                    <X size={12} />
                  </button>
                </div>
              ) : (
                <button
                  type="button"
                  className={`text-envibe-accent ${canEditPort ? 'cursor-pointer hover:underline hover:bg-envibe-bg-tertiary' : ''} flex items-center gap-1 px-1 py-0.5 rounded -mx-1`}
                  onClick={(e) => {
                    e.stopPropagation();
                    if (canEditPort) {
                      handleEditPort(e);
                    }
                  }}
                  disabled={!canEditPort}
                  title={canEditPort ? 'Click to edit port' : 'Stop service to edit port'}
                >
                  :{service.port || service.internalPort || '?'}
                  {service.internalPort && service.port && service.port !== service.internalPort && (
                    <span className="text-envibe-text-subtle">→{service.internalPort}</span>
                  )}
                  {canEditPort && <Edit2 size={10} className="opacity-50" />}
                </button>
              )
            )}
            {!isDockerService && service.port && (
              <span className="text-envibe-accent">:{service.port}</span>
            )}
            {service.image && (
              <span className="truncate">{service.image}</span>
            )}
            {service.command && (
              <span className="truncate font-mono">{service.command}</span>
            )}
          </div>

          {portError && (
            <div className="text-xs text-envibe-danger mt-1">
              {portError}
            </div>
          )}

          {service.errorMessage && (
            <div className="text-xs text-envibe-danger mt-1 truncate">
              {service.errorMessage}
            </div>
          )}

          <div className="flex items-center gap-1 mt-2">
            {service.status === 'stopped' || service.status === 'error' ? (
              <ActionButton
                icon={Play}
                label="Start"
                color="success"
                onClick={handleStart}
              />
            ) : service.status === 'running' ? (
              <>
                <ActionButton
                  icon={Square}
                  label="Stop"
                  color="danger"
                  onClick={handleStop}
                />
                <ActionButton
                  icon={RotateCw}
                  label="Restart"
                  color="warning"
                  onClick={handleRestart}
                />
              </>
            ) : null}
          </div>
        </div>
      </div>
    </li>
  );
}

interface ActionButtonProps {
  icon: React.ComponentType<{ size?: number }>;
  label: string;
  color: 'success' | 'danger' | 'warning';
  onClick: (e: React.MouseEvent) => void;
}

function ActionButton({ icon: Icon, label, color, onClick }: ActionButtonProps) {
  const colorClasses = {
    success: 'text-envibe-success hover:bg-envibe-success/20',
    danger: 'text-envibe-danger hover:bg-envibe-danger/20',
    warning: 'text-envibe-warning hover:bg-envibe-warning/20',
  };

  return (
    <button
      className={`p-1.5 rounded transition-colors ${colorClasses[color]}`}
      onClick={onClick}
      title={label}
    >
      <Icon size={14} />
    </button>
  );
}

function getTypeIcon(type: Service['type']) {
  switch (type) {
    case 'docker':
    case 'compose':
      return Database;
    case 'process':
      return TerminalIcon;
    default:
      return Server;
  }
}

function getStatusClass(status: ServiceStatus): string {
  switch (status) {
    case 'running':
      return 'status-dot-running';
    case 'starting':
    case 'stopping':
      return 'status-dot-starting';
    case 'error':
      return 'status-dot-error';
    default:
      return 'status-dot-stopped';
  }
}

function getStatusText(status: ServiceStatus): string {
  switch (status) {
    case 'running':
      return 'Running';
    case 'starting':
      return 'Starting';
    case 'stopping':
      return 'Stopping';
    case 'error':
      return 'Error';
    default:
      return 'Stopped';
  }
}

function getBadgeClass(status: ServiceStatus): string {
  switch (status) {
    case 'running':
      return 'badge-success';
    case 'starting':
    case 'stopping':
      return 'badge-warning';
    case 'error':
      return 'badge-danger';
    default:
      return 'badge-muted';
  }
}
