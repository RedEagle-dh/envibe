import { useState, useEffect } from 'react';
import { Modal } from './Modal';
import { useStore } from '../../stores/useStore';
import type { AIAgent } from '../../types';
import { Bot, Sparkles } from 'lucide-react';

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

export function SettingsModal() {
  const isOpen = useStore((s) => s.isSettingsModalOpen);
  const setSettingsModalOpen = useStore((s) => s.setSettingsModalOpen);
  const settings = useStore((s) => s.settings);
  const setSelectedAgents = useStore((s) => s.setSelectedAgents);

  const [localAgents, setLocalAgents] = useState<AIAgent[]>([]);

  useEffect(() => {
    if (isOpen) {
      setLocalAgents(settings.selectedAgents);
    }
  }, [isOpen, settings.selectedAgents]);

  const toggleAgent = (agentId: AIAgent) => {
    setLocalAgents((prev) =>
      prev.includes(agentId)
        ? prev.filter((id) => id !== agentId)
        : [...prev, agentId]
    );
  };

  const handleSave = () => {
    setSelectedAgents(localAgents);
    setSettingsModalOpen(false);
  };

  const handleCancel = () => {
    setSettingsModalOpen(false);
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={handleCancel}
      title="Settings"
      showCloseButton={true}
      closeOnBackdrop={true}
      closeOnEscape={true}
    >
      <div className="p-6">
        <div className="mb-4">
          <h3 className="text-sm font-medium text-envibe-text mb-3">AI Agents</h3>
          <div className="space-y-2">
            {AGENTS.map((agent) => {
              const Icon = agent.icon;
              const isSelected = localAgents.includes(agent.id);

              return (
                <button
                  key={agent.id}
                  onClick={() => toggleAgent(agent.id)}
                  className={`w-full p-3 rounded-lg border text-left transition-all ${
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

        <div className="flex gap-3 justify-end pt-4 border-t border-envibe-border">
          <button
            onClick={handleCancel}
            className="btn btn-ghost"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            className="btn btn-primary"
          >
            Save
          </button>
        </div>
      </div>
    </Modal>
  );
}
