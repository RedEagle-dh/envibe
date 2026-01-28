export type ServiceStatus = 'stopped' | 'starting' | 'running' | 'stopping' | 'error';
export type ServiceType = 'docker' | 'process' | 'compose' | 'agent';

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

export interface Project {
  name: string;
  path: string;
  hasDockerCompose: boolean;
  services: Service[];
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

export interface EnvibeAPI {
  getProjects: () => Promise<Project[]>;
  getServices: (projectName: string) => Promise<Service[]>;
  startService: (projectName: string, serviceName: string) => Promise<void>;
  stopService: (projectName: string, serviceName: string) => Promise<void>;
  restartService: (projectName: string, serviceName: string) => Promise<void>;
  setServicePort: (projectName: string, serviceName: string, port: number) => Promise<{ status: string; port?: number }>;
  addProject: () => Promise<{ status: string } | null>;
  removeProject: (projectPath: string) => Promise<{ status: string } | null>;
  getEnvVars: (projectName: string, serviceName?: string) => Promise<Record<string, string>>;
  getBackendUrl: () => Promise<string>;
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
