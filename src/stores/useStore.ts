import { create } from 'zustand';
import type { Project, LogEntry, ServiceStatus } from '../types';

interface AppState {
  // Projects
  projects: Project[];
  selectedProjectName: string | null;
  selectedServiceName: string | null;

  // Logs — stored per service for O(1) lookup (no filtering needed)
  logsByService: Record<string, LogEntry[]>;
  followLogs: boolean;

  // UI State
  showEnvPanel: boolean;

  // Actions
  setProjects: (projects: Project[]) => void;
  refreshProjects: () => Promise<void>;
  addProject: () => Promise<void>;
  removeProject: (projectPath: string) => Promise<void>;
  selectProject: (projectName: string | null) => void;
  selectService: (serviceName: string | null) => void;
  updateServiceStatus: (projectName: string, serviceName: string, status: ServiceStatus, port?: number) => void;

  addLogs: (entries: LogEntry[]) => void;
  clearLogs: () => void;
  clearServiceLogs: (serviceName: string) => void;
  setFollowLogs: (follow: boolean) => void;

  setShowEnvPanel: (show: boolean) => void;
}

const MAX_PER_SERVICE = 2000;
let logIdCounter = 0;

export const useStore = create<AppState>((set) => ({
  // Initial state
  projects: [],
  selectedProjectName: null,
  selectedServiceName: null,
  logsByService: {},
  followLogs: true,
  showEnvPanel: false,

  // Project actions
  setProjects: (projects) => set({ projects }),

  refreshProjects: async () => {
    if (window.envibe) {
      const projects = await window.envibe.getProjects();
      set({ projects });
    }
  },

  addProject: async () => {
    if (window.envibe) {
      const result = await window.envibe.addProject();
      if (result) {
        const projects = await window.envibe.getProjects();
        set({ projects });
      }
    }
  },

  removeProject: async (projectPath: string) => {
    if (window.envibe) {
      const result = await window.envibe.removeProject(projectPath);
      if (result) {
        const projects = await window.envibe.getProjects();
        set((state) => ({
          projects,
          selectedProjectName: state.projects.find(
            (p) => p.path === projectPath && p.name === state.selectedProjectName
          ) ? null : state.selectedProjectName,
          selectedServiceName: state.projects.find(
            (p) => p.path === projectPath && p.name === state.selectedProjectName
          ) ? null : state.selectedServiceName,
        }));
      }
    }
  },

  selectProject: (projectName) => set({
    selectedProjectName: projectName,
    selectedServiceName: null,
  }),

  selectService: (serviceName) => set({
    selectedServiceName: serviceName,
  }),

  updateServiceStatus: (projectName, serviceName, status, port) => set((state) => ({
    projects: state.projects.map((project) => {
      if (project.name !== projectName) return project;
      return {
        ...project,
        services: project.services.map((service) => {
          if (service.name !== serviceName) return service;
          return { ...service, status, port: port ?? service.port };
        }),
      };
    }),
  })),

  // Log actions — per-service buckets, only affected buckets get new references
  addLogs: (entries) => set((state) => {
    if (entries.length === 0) return state;

    // Group entries by service key
    const groups = new Map<string, LogEntry[]>();
    for (const entry of entries) {
      const key = entry.service ?? '__system__';
      let group = groups.get(key);
      if (!group) {
        group = [];
        groups.set(key, group);
      }
      group.push(entry);
    }

    // Only create new references for affected service buckets
    const updated = { ...state.logsByService };
    for (const [key, newEntries] of groups) {
      const existing = updated[key] ?? [];
      const combined = existing.concat(newEntries);
      updated[key] = combined.length > MAX_PER_SERVICE
        ? combined.slice(combined.length - MAX_PER_SERVICE)
        : combined;
    }

    return { logsByService: updated };
  }),

  clearLogs: () => set({ logsByService: {} }),

  clearServiceLogs: (serviceName) => set((state) => {
    const updated = { ...state.logsByService };
    delete updated[serviceName];
    return { logsByService: updated };
  }),

  setFollowLogs: (follow) => set({ followLogs: follow }),

  // UI actions
  setShowEnvPanel: (show) => set({ showEnvPanel: show }),
}));

// Strip ANSI escape codes
const ANSI_RE = /\x1b\[[0-9;]*m/g;

// Cached Intl formatter — avoids recreating on every call
const timeFmt = new Intl.DateTimeFormat('en-US', {
  hour12: false,
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
});

// Helper to create log entries
export function createLogEntry(
  message: string,
  options: {
    service?: string;
    project?: string;
    level?: LogEntry['level'];
  } = {}
): LogEntry {
  return {
    id: ++logIdCounter,
    time: timeFmt.format(new Date()),
    service: options.service,
    project: options.project,
    level: options.level ?? 'info',
    message: message.replace(ANSI_RE, ''),
  };
}

// Helper to parse ANSI and detect log level
export function parseLogLine(raw: string): { level: LogEntry['level']; message: string } {
  const lower = raw.toLowerCase();

  let level: LogEntry['level'] = 'info';
  if (lower.includes('error') || lower.includes('err]')) {
    level = 'error';
  } else if (lower.includes('warn') || lower.includes('warning')) {
    level = 'warn';
  } else if (lower.includes('debug') || lower.includes('dbg]')) {
    level = 'debug';
  }

  return { level, message: raw };
}

// Selectors
export const useSelectedProject = () => {
  const projects = useStore((s) => s.projects);
  const selectedName = useStore((s) => s.selectedProjectName);
  return projects.find((p) => p.name === selectedName) ?? null;
};

export const useSelectedService = () => {
  const project = useSelectedProject();
  const selectedName = useStore((s) => s.selectedServiceName);
  return project?.services.find((s) => s.name === selectedName) ?? null;
};

const EMPTY_LOGS: LogEntry[] = [];

// Returns logs for the selected service — O(1) lookup, no filter needed.
// Zustand only triggers re-render when this specific bucket's reference changes,
// NOT when other services' logs change.
export const useServiceLogs = () => {
  const selectedService = useStore((s) => s.selectedServiceName);
  return useStore((s) =>
    selectedService ? (s.logsByService[selectedService] ?? EMPTY_LOGS) : EMPTY_LOGS
  );
};
