import { GitBranch, Trash2 } from 'lucide-react';
import { useStore } from '../stores/useStore';
import type { Snapshot } from '../types';

interface SnapshotItemProps {
  snapshot: Snapshot;
  projectName: string;
  selected: boolean;
  onClick: () => void;
}

export function SnapshotItem({ snapshot, projectName, selected, onClick }: SnapshotItemProps) {
  const deleteSnapshot = useStore((s) => s.deleteSnapshot);
  const runningServices = snapshot.services.filter((s) => s.status === 'running').length;
  const totalServices = snapshot.services.length;
  const hasRunning = runningServices > 0;

  const handleDelete = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await deleteSnapshot(projectName, snapshot.id);
  };

  return (
    <li
      className={`list-item group pl-8 ${selected ? 'selected' : ''}`}
      onClick={onClick}
    >
      <div className="flex items-start gap-3">
        <div className={`status-dot mt-1.5 ${hasRunning ? 'status-dot-running' : 'status-dot-stopped'}`} />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <GitBranch size={12} className="text-envibe-text-muted flex-shrink-0" />
            <span className="font-medium text-sm truncate">{snapshot.name}</span>
            <button
              className="ml-auto p-1 rounded text-envibe-text-muted hover:text-envibe-danger hover:bg-envibe-danger/20 transition-colors opacity-0 group-hover:opacity-100 flex-shrink-0"
              onClick={handleDelete}
              title="Delete snapshot"
            >
              <Trash2 size={12} />
            </button>
          </div>
          <div className="text-xs text-envibe-text-muted mt-0.5 truncate flex items-center gap-1">
            <span className="text-envibe-accent">{snapshot.branch}</span>
          </div>
          {totalServices > 0 && (
            <div className="flex items-center gap-2 mt-1">
              <span className={`text-xs ${hasRunning ? 'text-envibe-success' : 'text-envibe-text-subtle'}`}>
                {runningServices}/{totalServices} services
              </span>
            </div>
          )}
        </div>
      </div>
    </li>
  );
}
