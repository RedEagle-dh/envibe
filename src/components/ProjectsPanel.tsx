import { useState, useRef, useEffect } from 'react';
import { FolderGit2, Package, Plus, X, FolderPlus, FolderOpen, ChevronDown, ChevronRight, GitBranch } from 'lucide-react';
import { useStore, useProjectExpanded, useProjectAgentStatus } from '../stores/useStore';
import { SnapshotItem } from './SnapshotItem';
import type { Project } from '../types';

function AddProjectDropdown() {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const addProject = useStore((s) => s.addProject);
  const setCreateProjectModalOpen = useStore((s) => s.setCreateProjectModalOpen);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [isOpen]);

  const handleAddExisting = () => {
    setIsOpen(false);
    addProject();
  };

  const handleCreateNew = () => {
    setIsOpen(false);
    setCreateProjectModalOpen(true);
  };

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        className="p-1 rounded text-envibe-text-muted hover:text-envibe-accent hover:bg-envibe-accent/20 transition-colors flex items-center gap-0.5"
        onClick={() => setIsOpen(!isOpen)}
        title="Add project"
      >
        <Plus size={16} />
        <ChevronDown size={12} />
      </button>
      {isOpen && (
        <div className="absolute right-0 top-full mt-1 w-48 bg-envibe-bg-secondary border border-envibe-border rounded-lg shadow-xl z-50 overflow-hidden">
          <button
            onClick={handleAddExisting}
            className="w-full px-3 py-2 text-left text-sm text-envibe-text hover:bg-envibe-accent/10 flex items-center gap-2 transition-colors"
          >
            <FolderOpen size={16} className="text-envibe-text-muted" />
            Add Existing Project
          </button>
          <button
            onClick={handleCreateNew}
            className="w-full px-3 py-2 text-left text-sm text-envibe-text hover:bg-envibe-accent/10 flex items-center gap-2 transition-colors"
          >
            <FolderPlus size={16} className="text-envibe-text-muted" />
            Create New Project
          </button>
        </div>
      )}
    </div>
  );
}

export function ProjectsPanel() {
  const projects = useStore((s) => s.projects);
  const selectedProjectName = useStore((s) => s.selectedProjectName);
  const selectedSnapshotId = useStore((s) => s.selectedSnapshotId);
  const selectProject = useStore((s) => s.selectProject);
  const selectSnapshot = useStore((s) => s.selectSnapshot);
  const addProject = useStore((s) => s.addProject);
  const setCreateProjectModalOpen = useStore((s) => s.setCreateProjectModalOpen);

  return (
    <div className="panel h-full flex flex-col">
      <div className="panel-header">
        <span className="panel-title flex items-center gap-2">
          <FolderGit2 size={16} className="text-envibe-accent" />
          Projects
        </span>
        <AddProjectDropdown />
      </div>
      <div className="panel-content flex-1 overflow-y-auto">
        {projects.length === 0 ? (
          <div className="p-4 text-center text-envibe-text-muted">
            <Package size={32} className="mx-auto mb-2 opacity-50" />
            <p className="text-sm">No projects added</p>
            <div className="mt-3 flex flex-col gap-2 items-center">
              <button
                className="px-3 py-1.5 text-xs rounded bg-envibe-accent/20 text-envibe-accent hover:bg-envibe-accent/30 transition-colors flex items-center gap-1.5"
                onClick={addProject}
              >
                <FolderOpen size={14} />
                Add Existing
              </button>
              <button
                className="px-3 py-1.5 text-xs rounded bg-envibe-bg-tertiary text-envibe-text-muted hover:text-envibe-text hover:bg-envibe-border transition-colors flex items-center gap-1.5"
                onClick={() => setCreateProjectModalOpen(true)}
              >
                <FolderPlus size={14} />
                Create New
              </button>
            </div>
          </div>
        ) : (
          <ul>
            {projects.map((project) => (
              <ProjectItem
                key={project.name}
                project={project}
                selected={project.name === selectedProjectName && !selectedSnapshotId}
                selectedSnapshotId={project.name === selectedProjectName ? selectedSnapshotId : null}
                onSelectProject={() => selectProject(project.name)}
                onSelectSnapshot={(snapshotId) => {
                  if (selectedProjectName !== project.name) {
                    selectProject(project.name);
                  }
                  selectSnapshot(snapshotId);
                }}
              />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

interface ProjectItemProps {
  project: Project;
  selected: boolean;
  selectedSnapshotId: string | null;
  onSelectProject: () => void;
  onSelectSnapshot: (snapshotId: string) => void;
}

function ProjectItem({ project, selected, selectedSnapshotId, onSelectProject, onSelectSnapshot }: ProjectItemProps) {
  const removeProject = useStore((s) => s.removeProject);
  const toggleProjectExpanded = useStore((s) => s.toggleProjectExpanded);
  const setCreateSnapshotModalOpen = useStore((s) => s.setCreateSnapshotModalOpen);
  const selectProject = useStore((s) => s.selectProject);
  const isExpanded = useProjectExpanded(project.name);
  const agentStatus = useProjectAgentStatus(project.name);
  const needsAgentAttention = agentStatus === 'waiting_input' || agentStatus === 'completed';

  const snapshots = project.snapshots ?? [];
  const hasSnapshots = snapshots.length > 0;

  // Count running services across project and all snapshots
  const projectRunning = project.services.filter((s) => s.status === 'running').length;
  const snapshotRunning = snapshots.reduce(
    (acc, snap) => acc + snap.services.filter((s) => s.status === 'running').length,
    0
  );
  const totalRunning = projectRunning + snapshotRunning;
  const totalServices = project.services.length + snapshots.reduce((acc, snap) => acc + snap.services.length, 0);
  const hasRunning = totalRunning > 0;

  const handleToggleExpand = (e: React.MouseEvent) => {
    e.stopPropagation();
    toggleProjectExpanded(project.name);
  };

  const handleAddSnapshot = (e: React.MouseEvent) => {
    e.stopPropagation();
    // Make sure project is selected first
    selectProject(project.name);
    setCreateSnapshotModalOpen(true);
  };

  const handleProjectClick = () => {
    onSelectProject();
    // Auto-expand when selecting
    if (!isExpanded && hasSnapshots) {
      toggleProjectExpanded(project.name);
    }
  };

  return (
    <>
      <li
        className={`list-item group ${selected ? 'selected' : ''} ${needsAgentAttention ? 'agent-attention' : ''}`}
        onClick={handleProjectClick}
      >
        <div className="flex items-start gap-2">
          {/* Expand/collapse button */}
          <button
            className="p-0.5 mt-1 rounded text-envibe-text-muted hover:text-envibe-text hover:bg-envibe-bg-tertiary transition-colors flex-shrink-0"
            onClick={handleToggleExpand}
          >
            {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          </button>

          <div className={`status-dot mt-1.5 ${hasRunning ? 'status-dot-running' : 'status-dot-stopped'}`} />

          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="font-medium text-sm truncate">{project.name}</span>
              {project.hasDockerCompose && (
                <span className="text-[10px] px-1.5 py-0.5 rounded bg-envibe-bg-tertiary text-envibe-text-subtle">
                  compose
                </span>
              )}
              <div className="ml-auto flex items-center gap-1 flex-shrink-0">
                <button
                  className="p-1 rounded text-envibe-text-muted hover:text-envibe-accent hover:bg-envibe-accent/20 transition-colors opacity-0 group-hover:opacity-100"
                  onClick={handleAddSnapshot}
                  title="Create snapshot"
                >
                  <GitBranch size={12} />
                </button>
                <button
                  className="p-1 rounded text-envibe-text-muted hover:text-envibe-danger hover:bg-envibe-danger/20 transition-colors opacity-0 group-hover:opacity-100"
                  onClick={(e) => {
                    e.stopPropagation();
                    removeProject(project.path);
                  }}
                  title="Remove project"
                >
                  <X size={12} />
                </button>
              </div>
            </div>
            <div className="text-xs text-envibe-text-muted mt-0.5 truncate">
              {project.path}
            </div>
            <div className="flex items-center gap-2 mt-1.5">
              <span className={`text-xs ${hasRunning ? 'text-envibe-success' : 'text-envibe-text-subtle'}`}>
                {totalRunning}/{totalServices} services
              </span>
              {hasSnapshots && (
                <span className="text-xs text-envibe-text-subtle">
                  {snapshots.length} snapshot{snapshots.length !== 1 ? 's' : ''}
                </span>
              )}
            </div>
          </div>
        </div>
      </li>

      {/* Nested snapshots */}
      {isExpanded && snapshots.map((snapshot) => (
        <SnapshotItem
          key={snapshot.id}
          snapshot={snapshot}
          projectName={project.name}
          selected={selectedSnapshotId === snapshot.id}
          onClick={() => onSelectSnapshot(snapshot.id)}
        />
      ))}

      {/* Add snapshot hint when expanded but no snapshots */}
      {isExpanded && !hasSnapshots && (
        <li className="pl-10 py-2 text-xs text-envibe-text-subtle">
          <button
            className="flex items-center gap-1.5 hover:text-envibe-accent transition-colors"
            onClick={handleAddSnapshot}
          >
            <Plus size={12} />
            Create a snapshot
          </button>
        </li>
      )}
    </>
  );
}
