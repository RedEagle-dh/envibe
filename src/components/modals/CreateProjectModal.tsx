import { useState, useEffect } from 'react';
import { Modal } from './Modal';
import { useStore } from '../../stores/useStore';
import type { AIAgent } from '../../types';
import { Bot, Sparkles, FolderOpen, AlertCircle } from 'lucide-react';

const AGENTS: { id: AIAgent; name: string; description: string; icon: typeof Bot }[] = [
  {
    id: 'claude-code',
    name: 'Claude Code',
    description: "Anthropic's AI coding assistant",
    icon: Sparkles,
  },
  {
    id: 'codex',
    name: 'Codex',
    description: "OpenAI's code generation model",
    icon: Bot,
  },
];

export function CreateProjectModal() {
  const isOpen = useStore((s) => s.isCreateProjectModalOpen);
  const setCreateProjectModalOpen = useStore((s) => s.setCreateProjectModalOpen);
  const settings = useStore((s) => s.settings);
  const createProject = useStore((s) => s.createProject);

  const [parentPath, setParentPath] = useState('');
  const [projectName, setProjectName] = useState('');
  const [selectedAgents, setSelectedAgents] = useState<AIAgent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  // Reset form when modal opens, pre-populate agents from settings
  useEffect(() => {
    if (isOpen) {
      setParentPath('');
      setProjectName('');
      setSelectedAgents(settings.selectedAgents);
      setError(null);
      setIsCreating(false);
    }
  }, [isOpen, settings.selectedAgents]);

  const handleSelectDirectory = async () => {
    if (!window.envibe) return;
    const selected = await window.envibe.selectDirectory('Select Parent Directory');
    if (selected) {
      setParentPath(selected);
      setError(null);
    }
  };

  const toggleAgent = (agentId: AIAgent) => {
    setSelectedAgents((prev) =>
      prev.includes(agentId)
        ? prev.filter((id) => id !== agentId)
        : [...prev, agentId]
    );
  };

  const handleCreate = async () => {
    if (!parentPath) {
      setError('Please select a parent directory');
      return;
    }
    if (!projectName.trim()) {
      setError('Please enter a project name');
      return;
    }

    setIsCreating(true);
    setError(null);

    const result = await createProject(parentPath, projectName.trim(), selectedAgents);

    if (!result.success) {
      setError(result.error ?? 'Failed to create project');
      setIsCreating(false);
    }
  };

  const handleClose = () => {
    if (!isCreating) {
      setCreateProjectModalOpen(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={handleClose}
      title="Create New Project"
      showCloseButton={!isCreating}
      closeOnBackdrop={!isCreating}
      closeOnEscape={!isCreating}
    >
      <div className="p-6 space-y-5">
        {/* Parent Directory */}
        <div>
          <label className="block text-sm font-medium text-envibe-text mb-2">
            Parent Directory
          </label>
          <div className="flex gap-2">
            <input
              type="text"
              value={parentPath}
              readOnly
              placeholder="Select a directory..."
              className="flex-1 px-3 py-2 rounded-lg bg-envibe-bg-tertiary border border-envibe-border text-envibe-text text-sm placeholder:text-envibe-text-muted focus:outline-none focus:border-envibe-accent"
            />
            <button
              onClick={handleSelectDirectory}
              disabled={isCreating}
              className="btn btn-ghost flex items-center gap-2"
            >
              <FolderOpen size={16} />
              Browse
            </button>
          </div>
        </div>

        {/* Project Name */}
        <div>
          <label className="block text-sm font-medium text-envibe-text mb-2">
            Project Name
          </label>
          <input
            type="text"
            value={projectName}
            onChange={(e) => {
              setProjectName(e.target.value);
              setError(null);
            }}
            placeholder="my-project"
            disabled={isCreating}
            className="w-full px-3 py-2 rounded-lg bg-envibe-bg-tertiary border border-envibe-border text-envibe-text text-sm placeholder:text-envibe-text-muted focus:outline-none focus:border-envibe-accent disabled:opacity-50"
          />
          {parentPath && projectName && (
            <p className="mt-1.5 text-xs text-envibe-text-muted">
              Will create: {parentPath}/{projectName.replace(/[<>:"/\\|?*]/g, '-').trim()}
            </p>
          )}
        </div>

        {/* Agent Selection */}
        <div>
          <label className="block text-sm font-medium text-envibe-text mb-2">
            AI Agents (optional)
          </label>
          <p className="text-xs text-envibe-text-muted mb-3">
            Pre-configure agents in the .envibe.yaml file
          </p>
          <div className="space-y-2">
            {AGENTS.map((agent) => {
              const Icon = agent.icon;
              const isSelected = selectedAgents.includes(agent.id);

              return (
                <button
                  key={agent.id}
                  onClick={() => toggleAgent(agent.id)}
                  disabled={isCreating}
                  className={`w-full p-3 rounded-lg border text-left transition-all disabled:opacity-50 ${
                    isSelected
                      ? 'border-envibe-accent bg-envibe-accent/10'
                      : 'border-envibe-border bg-envibe-bg-tertiary hover:border-envibe-border-muted'
                  }`}
                >
                  <div className="flex items-start gap-3">
                    <div className={`mt-0.5 w-5 h-5 rounded border flex items-center justify-center flex-shrink-0 ${
                      isSelected
                        ? 'bg-envibe-accent border-envibe-accent'
                        : 'border-envibe-text-muted'
                    }`}>
                      {isSelected && (
                        <svg className="w-3 h-3 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                          <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                        </svg>
                      )}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <Icon size={16} className={isSelected ? 'text-envibe-accent' : 'text-envibe-text-muted'} />
                        <span className="font-medium text-envibe-text">{agent.name}</span>
                      </div>
                      <p className="text-sm text-envibe-text-muted mt-0.5">
                        {agent.description}
                      </p>
                    </div>
                  </div>
                </button>
              );
            })}
          </div>
        </div>

        {/* Error Message */}
        {error && (
          <div className="flex items-center gap-2 p-3 rounded-lg bg-envibe-danger/10 border border-envibe-danger/30 text-envibe-danger text-sm">
            <AlertCircle size={16} className="flex-shrink-0" />
            {error}
          </div>
        )}

        {/* Actions */}
        <div className="flex gap-3 justify-end pt-4 border-t border-envibe-border">
          <button
            onClick={handleClose}
            disabled={isCreating}
            className="btn btn-ghost"
          >
            Cancel
          </button>
          <button
            onClick={handleCreate}
            disabled={isCreating || !parentPath || !projectName.trim()}
            className="btn btn-primary"
          >
            {isCreating ? 'Creating...' : 'Create Project'}
          </button>
        </div>
      </div>
    </Modal>
  );
}
