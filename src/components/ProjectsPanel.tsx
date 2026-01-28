import { FolderGit2, Package, Plus, X } from 'lucide-react';
import { useStore } from '../stores/useStore';
import type { Project } from '../types';

export function ProjectsPanel() {
  const projects = useStore((s) => s.projects);
  const selectedProjectName = useStore((s) => s.selectedProjectName);
  const selectProject = useStore((s) => s.selectProject);
  const addProject = useStore((s) => s.addProject);

  return (
    <div className="panel h-full flex flex-col">
      <div className="panel-header">
        <span className="panel-title flex items-center gap-2">
          <FolderGit2 size={16} className="text-envibe-accent" />
          Projects
        </span>
        <button
          className="p-1 rounded text-envibe-text-muted hover:text-envibe-accent hover:bg-envibe-accent/20 transition-colors"
          onClick={addProject}
          title="Add project"
        >
          <Plus size={16} />
        </button>
      </div>
      <div className="panel-content flex-1 overflow-y-auto">
        {projects.length === 0 ? (
          <div className="p-4 text-center text-envibe-text-muted">
            <Package size={32} className="mx-auto mb-2 opacity-50" />
            <p className="text-sm">No projects added</p>
            <button
              className="mt-3 px-3 py-1.5 text-xs rounded bg-envibe-accent/20 text-envibe-accent hover:bg-envibe-accent/30 transition-colors"
              onClick={addProject}
            >
              Add Project
            </button>
          </div>
        ) : (
          <ul>
            {projects.map((project) => (
              <ProjectItem
                key={project.name}
                project={project}
                selected={project.name === selectedProjectName}
                onClick={() => selectProject(project.name)}
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
  onClick: () => void;
}

function ProjectItem({ project, selected, onClick }: ProjectItemProps) {
  const removeProject = useStore((s) => s.removeProject);
  const runningServices = project.services.filter((s) => s.status === 'running').length;
  const totalServices = project.services.length;
  const hasRunning = runningServices > 0;

  return (
    <li
      className={`list-item group ${selected ? 'selected' : ''}`}
      onClick={onClick}
    >
      <div className="flex items-start gap-3">
        <div className={`status-dot mt-1.5 ${hasRunning ? 'status-dot-running' : 'status-dot-stopped'}`} />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-medium text-sm truncate">{project.name}</span>
            {project.hasDockerCompose && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-envibe-bg-tertiary text-envibe-text-subtle">
                compose
              </span>
            )}
            <button
              className="ml-auto p-1 rounded text-envibe-text-muted hover:text-envibe-danger hover:bg-envibe-danger/20 transition-colors opacity-0 group-hover:opacity-100 flex-shrink-0"
              onClick={(e) => {
                e.stopPropagation();
                removeProject(project.path);
              }}
              title="Remove project"
            >
              <X size={12} />
            </button>
          </div>
          <div className="text-xs text-envibe-text-muted mt-0.5 truncate">
            {project.path}
          </div>
          <div className="flex items-center gap-2 mt-1.5">
            <span className={`text-xs ${hasRunning ? 'text-envibe-success' : 'text-envibe-text-subtle'}`}>
              {runningServices}/{totalServices} services
            </span>
          </div>
        </div>
      </div>
    </li>
  );
}
