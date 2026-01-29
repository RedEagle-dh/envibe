import { useState } from 'react';
import { FolderGit } from 'lucide-react';
import { Modal } from './Modal';
import { useStore, useSelectedProject } from '../../stores/useStore';

export function CreateSnapshotModal() {
  const isOpen = useStore((s) => s.isCreateSnapshotModalOpen);
  const setOpen = useStore((s) => s.setCreateSnapshotModalOpen);
  const createSnapshot = useStore((s) => s.createSnapshot);
  const selectedProject = useSelectedProject();

  const [name, setName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const handleClose = () => {
    setName('');
    setError(null);
    setOpen(false);
  };

  const handleCreate = async () => {
    if (!selectedProject) {
      setError('No project selected');
      return;
    }

    if (!name.trim()) {
      setError('Snapshot name is required');
      return;
    }

    setError(null);
    setIsCreating(true);

    // Pass empty string for branch - backend will use current branch/HEAD
    const result = await createSnapshot(selectedProject.name, name.trim(), '');

    setIsCreating(false);

    if (!result.success) {
      setError(result.error ?? 'Failed to create snapshot');
      return;
    }

    handleClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !isCreating) {
      handleCreate();
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={handleClose} title="Create Snapshot">
      <div className="p-6 space-y-4">
        <p className="text-sm text-envibe-text-muted">
          Create a git worktree snapshot from the current state for isolated parallel development.
        </p>

        {selectedProject && (
          <div className="flex items-center gap-2 text-sm text-envibe-text-muted bg-envibe-bg-tertiary px-3 py-2 rounded">
            <FolderGit size={14} />
            <span>Project:</span>
            <span className="text-envibe-text font-medium">{selectedProject.name}</span>
          </div>
        )}

        <div>
          <label className="block text-sm font-medium text-envibe-text mb-1.5">
            Snapshot Name
          </label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="e.g., experiment-1, refactor-api"
            className="w-full bg-envibe-bg-tertiary border border-envibe-border rounded px-3 py-2 text-sm text-envibe-text placeholder:text-envibe-text-subtle focus:outline-none focus:border-envibe-accent"
            autoFocus
          />
          <p className="text-xs text-envibe-text-subtle mt-1">
            Creates a separate working directory from current HEAD
          </p>
        </div>

        {error && (
          <div className="text-sm text-envibe-danger bg-envibe-danger/10 px-3 py-2 rounded">
            {error}
          </div>
        )}

        <div className="flex justify-end gap-2 pt-2">
          <button
            onClick={handleClose}
            className="px-4 py-2 text-sm rounded bg-envibe-bg-tertiary text-envibe-text-muted hover:text-envibe-text hover:bg-envibe-border transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleCreate}
            disabled={isCreating || !name.trim()}
            className="px-4 py-2 text-sm rounded bg-envibe-accent text-white hover:bg-envibe-accent/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isCreating ? 'Creating...' : 'Create Snapshot'}
          </button>
        </div>
      </div>
    </Modal>
  );
}
