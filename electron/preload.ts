import { contextBridge, ipcRenderer } from 'electron';

export interface Snapshot {
  id: string;
  name: string;
  branch: string;
  path: string;
  createdAt: string;
  services: Service[];
  terminals: TerminalSession[];
}

export interface TerminalSession {
  id: string;
  name: string;
  status: 'connected' | 'disconnected';
  type: 'shell' | 'agent';
}

export interface EnvibeAPI {
  // Projects
  getProjects: () => Promise<Project[]>;

  // Services
  getServices: (projectName: string) => Promise<Service[]>;
  startService: (projectName: string, serviceName: string) => Promise<void>;
  stopService: (projectName: string, serviceName: string) => Promise<void>;
  restartService: (projectName: string, serviceName: string) => Promise<void>;
  setServicePort: (projectName: string, serviceName: string, port: number) => Promise<{ status: string; port?: number }>;

  // Project management
  addProject: () => Promise<{ status: string } | null>;
  removeProject: (projectPath: string) => Promise<{ status: string } | null>;
  selectDirectory: (title?: string) => Promise<string | null>;
  createProject: (parentPath: string, projectName: string, agents: string[]) => Promise<{ status: string; path: string } | { error: string }>;

  // Environment variables
  getEnvVars: (projectName: string, serviceName?: string) => Promise<Record<string, string>>;

  // Backend URL for WebSocket connections
  getBackendUrl: () => Promise<string>;

  // Snapshot management
  createSnapshot: (projectName: string, name: string, branch: string) => Promise<Snapshot | { error: string }>;
  deleteSnapshot: (projectName: string, snapshotId: string) => Promise<{ status: string } | { error: string }>;

  // Terminal management
  createTerminal: (projectName: string, snapshotId?: string, terminalType?: 'shell' | 'agent', agentCommand?: string) => Promise<TerminalSession | { error: string }>;
  closeTerminal: (terminalId: string) => Promise<{ status: string } | { error: string }>;

  // Events (batched for performance)
  onLog: (callback: (log: string) => void) => () => void;
  onLogs: (callback: (logs: string[]) => void) => () => void;
  onServiceUpdate: (callback: (update: ServiceUpdate) => void) => () => void;
  onRustError: (callback: (error: string) => void) => () => void;
  onRustErrors: (callback: (errors: string[]) => void) => () => void;
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

export interface Service {
  name: string;
  type: 'docker' | 'process' | 'compose';
  status: 'stopped' | 'starting' | 'running' | 'stopping' | 'error';
  port?: number;
  internalPort?: number;
  containerId?: string;
  processId?: number;
  errorMessage?: string;
}

export interface ServiceUpdate {
  project: string;
  service: string;
  status: Service['status'];
  port?: number;
}

const api: EnvibeAPI = {
  getProjects: () => ipcRenderer.invoke('get-projects'),
  getServices: (projectName) => ipcRenderer.invoke('get-services', projectName),
  startService: (projectName, serviceName) => ipcRenderer.invoke('start-service', projectName, serviceName),
  stopService: (projectName, serviceName) => ipcRenderer.invoke('stop-service', projectName, serviceName),
  restartService: (projectName, serviceName) => ipcRenderer.invoke('restart-service', projectName, serviceName),
  setServicePort: (projectName, serviceName, port) => ipcRenderer.invoke('set-service-port', projectName, serviceName, port),
  addProject: () => ipcRenderer.invoke('add-project'),
  removeProject: (projectPath) => ipcRenderer.invoke('remove-project', projectPath),
  selectDirectory: (title) => ipcRenderer.invoke('select-directory', title),
  createProject: (parentPath, projectName, agents) => ipcRenderer.invoke('create-project', parentPath, projectName, agents),
  getBackendUrl: () => ipcRenderer.invoke('get-backend-url'),
  getEnvVars: (projectName, serviceName) => ipcRenderer.invoke('get-env-vars', projectName, serviceName),

  // Snapshot management
  createSnapshot: (projectName, name, branch) => ipcRenderer.invoke('create-snapshot', projectName, name, branch),
  deleteSnapshot: (projectName, snapshotId) => ipcRenderer.invoke('delete-snapshot', projectName, snapshotId),

  // Terminal management
  createTerminal: (projectName, snapshotId, terminalType, agentCommand) => ipcRenderer.invoke('create-terminal', projectName, snapshotId, terminalType, agentCommand),
  closeTerminal: (terminalId) => ipcRenderer.invoke('close-terminal', terminalId),

  onLog: (callback) => {
    const handler = (_event: Electron.IpcRendererEvent, log: string) => callback(log);
    ipcRenderer.on('rust-log', handler);
    return () => ipcRenderer.removeListener('rust-log', handler);
  },

  onLogs: (callback) => {
    const handler = (_event: Electron.IpcRendererEvent, logs: string[]) => callback(logs);
    ipcRenderer.on('rust-logs', handler);
    return () => ipcRenderer.removeListener('rust-logs', handler);
  },

  onServiceUpdate: (callback) => {
    const handler = (_event: Electron.IpcRendererEvent, update: ServiceUpdate) => callback(update);
    ipcRenderer.on('service-update', handler);
    return () => ipcRenderer.removeListener('service-update', handler);
  },

  onRustError: (callback) => {
    const handler = (_event: Electron.IpcRendererEvent, error: string) => callback(error);
    ipcRenderer.on('rust-error', handler);
    return () => ipcRenderer.removeListener('rust-error', handler);
  },

  onRustErrors: (callback) => {
    const handler = (_event: Electron.IpcRendererEvent, errors: string[]) => callback(errors);
    ipcRenderer.on('rust-errors', handler);
    return () => ipcRenderer.removeListener('rust-errors', handler);
  },
};

contextBridge.exposeInMainWorld('envibe', api);
