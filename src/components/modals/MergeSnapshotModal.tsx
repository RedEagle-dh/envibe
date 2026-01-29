import { useState } from 'react';
import { GitMerge, AlertTriangle } from 'lucide-react';
import { Modal } from './Modal';
import { useStore } from '../../stores/useStore';

export function MergeSnapshotModal() {
  const isOpen = useStore((s) => s.isMergeSnapshotModalOpen);
  const setOpen = useStore((s) => s.setMergeSnapshotModalOpen);
  const mergeTargetSnapshot = useStore((s) => s.mergeTargetSnapshot);
  const mergeSnapshot = useStore((s) => s.mergeSnapshot);

  const [commitMessage, setCommitMessage] = useState('');
  const [deleteAfterMerge, setDeleteAfterMerge] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [conflictFiles, setConflictFiles] = useState<string[]>([]);
  const [isMerging, setIsMerging] = useState(false);

  const handleClose = () => {
    setCommitMessage('');
    setDeleteAfterMerge(true);
    setError(null);
    setConflictFiles([]);
    setOpen(false);
  };

  const handleMerge = async () => {
    if (!mergeTargetSnapshot) {
      setError('No snapshot selected');
      return;
    }

    setError(null);
    setConflictFiles([]);
    setIsMerging(true);

    const result = await mergeSnapshot(
      mergeTargetSnapshot.projectName,
      mergeTargetSnapshot.snapshotId,
      deleteAfterMerge,
      commitMessage.trim() || undefined
    );

    setIsMerging(false);

    if (!result.success) {
      setError(result.message);
      if (result.hasConflicts && result.conflictFiles.length > 0) {
        setConflictFiles(result.conflictFiles);
      }
      return;
    }

    handleClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey && !isMerging) {
      e.preventDefault();
      handleMerge();
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={handleClose} title="Merge Snapshot">
      <div className="p-6 space-y-4">
        {mergeTargetSnapshot && (
          <div className="flex items-center gap-2 text-sm text-envibe-text-muted bg-envibe-bg-tertiary px-3 py-2 rounded">
            <GitMerge size={14} className="text-envibe-accent" />
            <span>Merging:</span>
            <span className="text-envibe-text font-medium">{mergeTargetSnapshot.snapshotName}</span>
          </div>
        )}

        <div className="flex items-start gap-3 p-3 bg-envibe-warning/10 border border-envibe-warning/30 rounded">
          <AlertTriangle size={16} className="text-envibe-warning flex-shrink-0 mt-0.5" />
          <div className="text-sm">
            <p className="text-envibe-warning font-medium">Before merging</p>
            <p className="text-envibe-text-muted mt-1">
              Ensure all changes in both the main project and the snapshot are committed.
              Uncommitted changes will block the merge.
            </p>
          </div>
        </div>

        <div>
          <label className="block text-sm font-medium text-envibe-text mb-1.5">
            Commit Message <span className="text-envibe-text-muted font-normal">(optional)</span>
          </label>
          <input
            type="text"
            value={commitMessage}
            onChange={(e) => setCommitMessage(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={`Merge snapshot '${mergeTargetSnapshot?.snapshotName ?? ''}' into main branch`}
            className="w-full bg-envibe-bg-tertiary border border-envibe-border rounded px-3 py-2 text-sm text-envibe-text placeholder:text-envibe-text-subtle focus:outline-none focus:border-envibe-accent"
          />
        </div>

        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={deleteAfterMerge}
            onChange={(e) => setDeleteAfterMerge(e.target.checked)}
            className="w-4 h-4 rounded border-envibe-border bg-envibe-bg-tertiary text-envibe-accent focus:ring-envibe-accent focus:ring-offset-0"
          />
          <span className="text-sm text-envibe-text">Delete snapshot after successful merge</span>
        </label>

        {error && (
          <div className="text-sm text-envibe-danger bg-envibe-danger/10 px-3 py-2 rounded">
            <p>{error}</p>
            {conflictFiles.length > 0 && (
              <div className="mt-2">
                <p className="font-medium">Conflicting files:</p>
                <ul className="mt-1 list-disc list-inside text-envibe-text-muted">
                  {conflictFiles.map((file) => (
                    <li key={file} className="font-mono text-xs">{file}</li>
                  ))}
                </ul>
              </div>
            )}
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
            onClick={handleMerge}
            disabled={isMerging}
            className="px-4 py-2 text-sm rounded bg-envibe-accent text-white hover:bg-envibe-accent/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
          >
            <GitMerge size={14} />
            {isMerging ? 'Merging...' : 'Merge Snapshot'}
          </button>
        </div>
      </div>
    </Modal>
  );
}
