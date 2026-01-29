export type ServiceStatus = 'stopped' | 'starting' | 'running' | 'stopping' | 'error';
export type ServiceType = 'docker' | 'process' | 'compose';
export type AIAgent = 'claude-code' | 'codex';
export type TerminalStatus = 'connected' | 'disconnected';
export type TerminalType = 'shell' | 'agent';
export type AgentStatus = 'idle' | 'busy' | 'waiting_input' | 'completed';

export interface AppSettings {
  isFirstTimeSetupComplete: boolean;
  selectedAgents: AIAgent[];
}

export interface Service {
  name: string;
  type: ServiceType;
  status: ServiceStatus;
  port?: number;
  internalPort?: number;
  containerId?: string;
  processId?: number;
  errorMessage?: string;
  image?: string;
  command?: string;
}

export interface TerminalSession {
  id: string;
  name: string;
  status: TerminalStatus;
  type: TerminalType;
  agentStatus?: AgentStatus;
}

export interface Snapshot {
  id: string;
  name: string;
  branch: string;
  path: string;
  createdAt: string;
  services: Service[];
  terminals: TerminalSession[];
}

export interface Project {
  name: string;
  path: string;
  hasDockerCompose: boolean;
  services: Service[];
  terminals: TerminalSession[];
  snapshots: Snapshot[];
  isExpanded?: boolean;
}

export interface LogEntry {
  id: number;
  time: string;
  service?: string;
  project?: string;
  level: 'info' | 'warn' | 'error' | 'debug';
  message: string;
}

export interface EnvVar {
  key: string;
  value: string;
  source: 'env-file' | 'inline' | 'compose' | 'system';
  interpolated?: string;
}

export interface MergeResult {
  success: boolean;
  message: string;
  hasConflicts: boolean;
  conflictFiles: string[];
  commitHash?: string;
}

export interface EnvibeAPI {
  getProjects: () => Promise<Project[]>;
  getServices: (projectName: string) => Promise<Service[]>;
  startService: (projectName: string, serviceName: string) => Promise<void>;
  stopService: (projectName: string, serviceName: string) => Promise<void>;
  restartService: (projectName: string, serviceName: string) => Promise<void>;
  setServicePort: (projectName: string, serviceName: string, port: number) => Promise<{ status: string; port?: number }>;
  addProject: () => Promise<{ status: string } | null>;
  removeProject: (projectPath: string) => Promise<{ status: string } | null>;
  selectDirectory: (title?: string) => Promise<string | null>;
  createProject: (parentPath: string, projectName: string, agents: string[]) => Promise<{ status: string; path: string } | { error: string }>;
  getEnvVars: (projectName: string, serviceName?: string) => Promise<Record<string, string>>;
  getBackendUrl: () => Promise<string>;

  // Snapshot management
  createSnapshot: (projectName: string, name: string, branch: string) => Promise<Snapshot | { error: string }>;
  deleteSnapshot: (projectName: string, snapshotId: string) => Promise<{ status: string } | { error: string }>;
  mergeSnapshot: (projectName: string, snapshotId: string, deleteAfterMerge: boolean, commitMessage?: string) => Promise<MergeResult>;

  // Terminal management
  createTerminal: (projectName: string, snapshotId?: string, terminalType?: TerminalType, agentCommand?: string) => Promise<TerminalSession | { error: string }>;
  closeTerminal: (terminalId: string) => Promise<{ status: string } | { error: string }>;

  // Events (batched for performance)
  onLog: (callback: (log: string) => void) => () => void;
  onLogs: (callback: (logs: string[]) => void) => () => void;
  onServiceUpdate: (callback: (update: { project: string; service: string; status: ServiceStatus; port?: number }) => void) => () => void;
  onRustError: (callback: (error: string) => void) => () => void;
  onRustErrors: (callback: (errors: string[]) => void) => () => void;
}

declare global {
  interface Window {
    envibe: EnvibeAPI;
  }
}
