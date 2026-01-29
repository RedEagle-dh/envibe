import { useState } from 'react';
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

export function SetupModal() {
  const isOpen = useStore((s) => s.isSetupModalOpen);
  const completeFirstTimeSetup = useStore((s) => s.completeFirstTimeSetup);
  const setSelectedAgents = useStore((s) => s.setSelectedAgents);

  const [selectedAgents, setLocalSelectedAgents] = useState<AIAgent[]>([]);

  const toggleAgent = (agentId: AIAgent) => {
    setLocalSelectedAgents((prev) =>
      prev.includes(agentId)
        ? prev.filter((id) => id !== agentId)
        : [...prev, agentId]
    );
  };

  const handleGetStarted = () => {
    setSelectedAgents(selectedAgents);
    completeFirstTimeSetup();
  };

  return (
    <Modal
      isOpen={isOpen}
      showCloseButton={false}
      closeOnBackdrop={false}
      closeOnEscape={false}
    >
      <div className="p-6 text-center">
        <div className="mb-6">
          <h1 className="text-2xl font-bold text-envibe-text mb-2">
            Welcome to Envibe
          </h1>
          <p className="text-envibe-text-muted">
            Select your AI coding assistants
          </p>
        </div>

        <div className="space-y-3 mb-6">
          {AGENTS.map((agent) => {
            const Icon = agent.icon;
            const isSelected = selectedAgents.includes(agent.id);

            return (
              <button
                key={agent.id}
                onClick={() => toggleAgent(agent.id)}
                className={`w-full p-4 rounded-lg border text-left transition-all ${
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
                    <p className="text-sm text-envibe-text-muted mt-1">
                      {agent.description}
                    </p>
                  </div>
                </div>
              </button>
            );
          })}
        </div>

        <button
          onClick={handleGetStarted}
          className="btn btn-primary w-full py-2.5"
        >
          Get Started
        </button>
      </div>
    </Modal>
  );
}
