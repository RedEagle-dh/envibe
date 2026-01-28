import { useState, useEffect } from 'react';
import { KeyRound, Copy, Eye, EyeOff, X, Search, ChevronDown, ChevronRight } from 'lucide-react';
import { useStore, useSelectedProject, useSelectedService } from '../stores/useStore';
import type { EnvVar } from '../types';

// Mock environment variables for demo
const mockEnvVars: EnvVar[] = [
  { key: 'DATABASE_URL', value: 'postgres://dev:dev@localhost:5432/myapp', source: 'env-file', interpolated: 'postgres://dev:dev@localhost:5432/myapp' },
  { key: 'REDIS_URL', value: 'redis://localhost:${redis.port}', source: 'env-file', interpolated: 'redis://localhost:6379' },
  { key: 'PORT', value: '3000', source: 'inline' },
  { key: 'NODE_ENV', value: 'development', source: 'inline' },
  { key: 'JWT_SECRET', value: 'super-secret-key-do-not-share', source: 'env-file' },
  { key: 'API_KEY', value: 'sk-1234567890abcdef', source: 'env-file' },
  { key: 'DEBUG', value: 'true', source: 'inline' },
  { key: 'LOG_LEVEL', value: 'debug', source: 'compose' },
];

export function EnvPanel() {
  const selectedProject = useSelectedProject();
  const selectedService = useSelectedService();
  const setShowEnvPanel = useStore((s) => s.setShowEnvPanel);

  const [envVars, setEnvVars] = useState<EnvVar[]>([]);
  const [filter, setFilter] = useState('');
  const [showValues, setShowValues] = useState<Set<string>>(new Set());
  const [expandedSources, setExpandedSources] = useState<Set<string>>(new Set(['env-file', 'inline', 'compose']));

  useEffect(() => {
    // Fetch env vars from backend when project/service changes
    if (window.envibe && selectedProject) {
      window.envibe
        .getEnvVars(selectedProject.name, selectedService?.name)
        .then((vars) => {
          const envList = Object.entries(vars).map(([key, value]) => ({
            key,
            value,
            source: 'env-file' as const,
          }));
          setEnvVars(envList);
        })
        .catch(() => {
          // Clear vars on error
          setEnvVars([]);
        });
    } else if (!window.envibe) {
      // Only use mock data when no backend is available (dev without Electron)
      setEnvVars(mockEnvVars);
    } else {
      // No project selected
      setEnvVars([]);
    }
  }, [selectedProject, selectedService]);

  const filteredVars = filter
    ? envVars.filter((v) =>
        v.key.toLowerCase().includes(filter.toLowerCase()) ||
        v.value.toLowerCase().includes(filter.toLowerCase())
      )
    : envVars;

  const groupedVars = filteredVars.reduce((acc, v) => {
    if (!acc[v.source]) acc[v.source] = [];
    acc[v.source].push(v);
    return acc;
  }, {} as Record<string, EnvVar[]>);

  const toggleValue = (key: string) => {
    const newSet = new Set(showValues);
    if (newSet.has(key)) {
      newSet.delete(key);
    } else {
      newSet.add(key);
    }
    setShowValues(newSet);
  };

  const toggleSource = (source: string) => {
    const newSet = new Set(expandedSources);
    if (newSet.has(source)) {
      newSet.delete(source);
    } else {
      newSet.add(source);
    }
    setExpandedSources(newSet);
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
  };

  const sourceLabels: Record<string, string> = {
    'env-file': 'Environment Files',
    inline: 'Inline Variables',
    compose: 'Docker Compose',
    system: 'System',
  };

  return (
    <div className="panel h-full flex flex-col">
      <div className="panel-header">
        <span className="panel-title flex items-center gap-2">
          <KeyRound size={16} className="text-envibe-accent" />
          Environment
          {(selectedService || selectedProject) && (
            <span className="text-envibe-text-muted text-xs">
              ({selectedService?.name || selectedProject?.name})
            </span>
          )}
        </span>
        <button
          className="icon-btn"
          onClick={() => setShowEnvPanel(false)}
        >
          <X size={14} />
        </button>
      </div>

      <div className="px-3 py-2 border-b border-envibe-border">
        <div className="flex items-center gap-2 bg-envibe-bg-tertiary rounded-md px-2">
          <Search size={14} className="text-envibe-text-muted" />
          <input
            type="text"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Filter variables..."
            className="bg-transparent border-none text-sm text-envibe-text placeholder:text-envibe-text-subtle focus:outline-none flex-1 py-1.5"
          />
          {filter && (
            <button
              className="icon-btn p-0.5"
              onClick={() => setFilter('')}
            >
              <X size={14} />
            </button>
          )}
        </div>
      </div>

      <div className="panel-content flex-1 overflow-y-auto">
        {Object.entries(groupedVars).map(([source, vars]) => (
          <div key={source}>
            <button
              className="w-full px-3 py-2 flex items-center gap-2 text-xs text-envibe-text-muted hover:bg-envibe-bg-tertiary"
              onClick={() => toggleSource(source)}
            >
              {expandedSources.has(source) ? (
                <ChevronDown size={14} />
              ) : (
                <ChevronRight size={14} />
              )}
              <span className="font-medium">{sourceLabels[source] || source}</span>
              <span className="text-envibe-text-subtle">({vars.length})</span>
            </button>

            {expandedSources.has(source) && (
              <div className="space-y-1 px-2 pb-2">
                {vars.map((envVar) => (
                  <EnvVarRow
                    key={envVar.key}
                    envVar={envVar}
                    showValue={showValues.has(envVar.key)}
                    onToggleValue={() => toggleValue(envVar.key)}
                    onCopy={() => copyToClipboard(envVar.interpolated || envVar.value)}
                  />
                ))}
              </div>
            )}
          </div>
        ))}

        {filteredVars.length === 0 && (
          <div className="p-4 text-center text-envibe-text-muted">
            <KeyRound size={32} className="mx-auto mb-2 opacity-50" />
            <p className="text-sm">
              {filter
                ? 'No matching variables'
                : !selectedProject
                  ? 'Select a project to view environment variables'
                  : 'No environment variables found'}
            </p>
          </div>
        )}
      </div>

      <div className="px-3 py-2 border-t border-envibe-border text-xs text-envibe-text-subtle">
        {filteredVars.length} variable{filteredVars.length !== 1 ? 's' : ''}
      </div>
    </div>
  );
}

interface EnvVarRowProps {
  envVar: EnvVar;
  showValue: boolean;
  onToggleValue: () => void;
  onCopy: () => void;
}

function EnvVarRow({ envVar, showValue, onToggleValue, onCopy }: EnvVarRowProps) {
  const isSensitive = /secret|key|password|token|credential/i.test(envVar.key);
  const displayValue = showValue || !isSensitive
    ? (envVar.interpolated || envVar.value)
    : '••••••••••••';

  return (
    <div className="group bg-envibe-bg-tertiary/50 rounded-md p-2 hover:bg-envibe-bg-tertiary">
      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-xs text-envibe-accent truncate">
          {envVar.key}
        </span>
        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
          {isSensitive && (
            <button
              className="icon-btn p-0.5"
              onClick={onToggleValue}
              title={showValue ? 'Hide value' : 'Show value'}
            >
              {showValue ? <EyeOff size={12} /> : <Eye size={12} />}
            </button>
          )}
          <button
            className="icon-btn p-0.5"
            onClick={onCopy}
            title="Copy value"
          >
            <Copy size={12} />
          </button>
        </div>
      </div>
      <div className="font-mono text-xs text-envibe-text-muted mt-1 truncate">
        {displayValue}
      </div>
      {envVar.interpolated && envVar.interpolated !== envVar.value && (
        <div className="text-[10px] text-envibe-text-subtle mt-0.5">
          Raw: {envVar.value}
        </div>
      )}
    </div>
  );
}
