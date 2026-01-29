import { create } from 'zustand';
import type { Project, LogEntry, ServiceStatus, AppSettings, AIAgent, Snapshot, TerminalSession, Service, TerminalType, MergeResult } from '../types';
import { loadSettings, saveSettings } from '../hooks/useSettings';

interface MergeSnapshotTarget {
  projectName: string;
  snapshotId: string;
  snapshotName: string;
}

interface AppState {
  // Projects
  projects: Project[];
  selectedProjectName: string | null;
  selectedServiceName: string | null;

  // Snapshots & Terminals
  selectedSnapshotId: string | null;
  selectedTerminalId: string | null;
  openTerminals: string[];
  expandedProjects: Set<string>;

  // Logs — stored per service for O(1) lookup (no filtering needed)
  logsByService: Record<string, LogEntry[]>;
  followLogs: boolean;

  // UI State
  showEnvPanel: boolean;

  // Settings
  settings: AppSettings;
  isSettingsModalOpen: boolean;
  isSetupModalOpen: boolean;
  isCreateProjectModalOpen: boolean;
  isCreateSnapshotModalOpen: boolean;
  isMergeSnapshotModalOpen: boolean;
  mergeTargetSnapshot: MergeSnapshotTarget | null;

  // Actions
  setProjects: (projects: Project[]) => void;
  refreshProjects: () => Promise<void>;
  addProject: () => Promise<void>;
  removeProject: (projectPath: string) => Promise<void>;
  selectProject: (projectName: string | null) => void;
  selectService: (serviceName: string | null) => void;
  updateServiceStatus: (projectName: string, serviceName: string, status: ServiceStatus, port?: number) => void;

  // Snapshot actions
  selectSnapshot: (snapshotId: string | null) => void;
  toggleProjectExpanded: (projectName: string) => void;
  createSnapshot: (projectName: string, name: string, branch: string) => Promise<{ success: boolean; error?: string }>;
  deleteSnapshot: (projectName: string, snapshotId: string) => Promise<{ success: boolean; error?: string }>;
  setCreateSnapshotModalOpen: (open: boolean) => void;
  setMergeSnapshotModalOpen: (open: boolean, target?: MergeSnapshotTarget) => void;
  mergeSnapshot: (projectName: string, snapshotId: string, deleteAfterMerge: boolean, commitMessage?: string) => Promise<MergeResult>;

  // Terminal actions
  selectTerminal: (terminalId: string | null) => void;
  openTerminal: (terminalId: string) => void;
  closeTerminalTab: (terminalId: string) => void;
  createTerminal: (projectName: string, snapshotId?: string, terminalType?: TerminalType, agentCommand?: string) => Promise<{ success: boolean; terminalId?: string; error?: string }>;
  closeTerminal: (terminalId: string) => Promise<{ success: boolean; error?: string }>;

  addLogs: (entries: LogEntry[]) => void;
  clearLogs: () => void;
  clearServiceLogs: (serviceName: string) => void;
  setFollowLogs: (follow: boolean) => void;

  setShowEnvPanel: (show: boolean) => void;

  // Settings actions
  initSettings: () => void;
  setSettings: (settings: Partial<AppSettings>) => void;
  setSelectedAgents: (agents: AIAgent[]) => void;
  completeFirstTimeSetup: () => void;
  setSettingsModalOpen: (open: boolean) => void;
  setSetupModalOpen: (open: boolean) => void;
  setCreateProjectModalOpen: (open: boolean) => void;
  createProject: (parentPath: string, projectName: string, agents: AIAgent[]) => Promise<{ success: boolean; error?: string }>;
}

const MAX_PER_SERVICE = 2000;
let logIdCounter = 0;

export const useStore = create<AppState>((set, get) => ({
  // Initial state
  projects: [],
  selectedProjectName: null,
  selectedServiceName: null,
  selectedSnapshotId: null,
  selectedTerminalId: null,
  openTerminals: [],
  expandedProjects: new Set<string>(),
  logsByService: {},
  followLogs: true,
  showEnvPanel: false,

  // Settings state
  settings: {
    isFirstTimeSetupComplete: false,
    selectedAgents: [],
  },
  isSettingsModalOpen: false,
  isSetupModalOpen: false,
  isCreateProjectModalOpen: false,
  isCreateSnapshotModalOpen: false,
  isMergeSnapshotModalOpen: false,
  mergeTargetSnapshot: null,

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
          selectedSnapshotId: null,
          selectedTerminalId: null,
        }));
      }
    }
  },

  selectProject: (projectName) => set({
    selectedProjectName: projectName,
    selectedServiceName: null,
    selectedSnapshotId: null,
    selectedTerminalId: null,
  }),

  selectService: (serviceName) => set({
    selectedServiceName: serviceName,
  }),

  // Snapshot actions
  selectSnapshot: (snapshotId) => set({
    selectedSnapshotId: snapshotId,
    selectedServiceName: null,
    selectedTerminalId: null,
  }),

  toggleProjectExpanded: (projectName) => set((state) => {
    const newExpanded = new Set(state.expandedProjects);
    if (newExpanded.has(projectName)) {
      newExpanded.delete(projectName);
    } else {
      newExpanded.add(projectName);
    }
    return { expandedProjects: newExpanded };
  }),

  createSnapshot: async (projectName, name, branch) => {
    if (!window.envibe) {
      return { success: false, error: 'API not available' };
    }

    const result = await window.envibe.createSnapshot(projectName, name, branch);

    if ('error' in result) {
      return { success: false, error: result.error };
    }

    // Refresh projects to get updated snapshot list
    const projects = await window.envibe.getProjects();
    set({ projects, isCreateSnapshotModalOpen: false });

    return { success: true };
  },

  deleteSnapshot: async (projectName, snapshotId) => {
    if (!window.envibe) {
      return { success: false, error: 'API not available' };
    }

    const result = await window.envibe.deleteSnapshot(projectName, snapshotId);

    if ('error' in result) {
      return { success: false, error: result.error };
    }

    // Refresh projects and clear selection if needed
    const projects = await window.envibe.getProjects();
    set((state) => ({
      projects,
      selectedSnapshotId: state.selectedSnapshotId === snapshotId ? null : state.selectedSnapshotId,
      selectedServiceName: state.selectedSnapshotId === snapshotId ? null : state.selectedServiceName,
      selectedTerminalId: state.selectedSnapshotId === snapshotId ? null : state.selectedTerminalId,
    }));

    return { success: true };
  },

  setCreateSnapshotModalOpen: (open) => set({ isCreateSnapshotModalOpen: open }),

  setMergeSnapshotModalOpen: (open, target) => set({
    isMergeSnapshotModalOpen: open,
    mergeTargetSnapshot: open ? target ?? null : null,
  }),

  mergeSnapshot: async (projectName, snapshotId, deleteAfterMerge, commitMessage) => {
    if (!window.envibe) {
      return { success: false, message: 'API not available', hasConflicts: false, conflictFiles: [] };
    }

    const result = await window.envibe.mergeSnapshot(projectName, snapshotId, deleteAfterMerge, commitMessage);

    // Refresh projects to reflect any changes (snapshot deletion, etc.)
    if (result.success) {
      const projects = await window.envibe.getProjects();
      set((state) => ({
        projects,
        // Clear snapshot selection if it was deleted
        selectedSnapshotId: deleteAfterMerge && state.selectedSnapshotId === snapshotId
          ? null
          : state.selectedSnapshotId,
      }));
    }

    return result;
  },

  // Terminal actions
  selectTerminal: (terminalId) => set({
    selectedTerminalId: terminalId,
    selectedServiceName: null,
  }),

  openTerminal: (terminalId) => set((state) => {
    if (state.openTerminals.includes(terminalId)) {
      return { selectedTerminalId: terminalId };
    }
    return {
      openTerminals: [...state.openTerminals, terminalId],
      selectedTerminalId: terminalId,
    };
  }),

  closeTerminalTab: (terminalId) => set((state) => {
    const newOpenTerminals = state.openTerminals.filter((id) => id !== terminalId);
    return {
      openTerminals: newOpenTerminals,
      selectedTerminalId: state.selectedTerminalId === terminalId
        ? (newOpenTerminals[newOpenTerminals.length - 1] ?? null)
        : state.selectedTerminalId,
    };
  }),

  createTerminal: async (projectName, snapshotId, terminalType = 'shell', agentCommand) => {
    if (!window.envibe) {
      return { success: false, error: 'API not available' };
    }

    const result = await window.envibe.createTerminal(projectName, snapshotId, terminalType, agentCommand);

    if ('error' in result) {
      return { success: false, error: result.error };
    }

    // Refresh projects to get updated terminal list
    const projects = await window.envibe.getProjects();
    set((state) => ({
      projects,
      openTerminals: [...state.openTerminals, result.id],
      selectedTerminalId: result.id,
      selectedServiceName: null,
    }));

    return { success: true, terminalId: result.id };
  },

  closeTerminal: async (terminalId) => {
    if (!window.envibe) {
      return { success: false, error: 'API not available' };
    }

    const result = await window.envibe.closeTerminal(terminalId);

    if ('error' in result) {
      return { success: false, error: result.error };
    }

    // Remove from open terminals and refresh projects
    const projects = await window.envibe.getProjects();
    set((state) => {
      const newOpenTerminals = state.openTerminals.filter((id) => id !== terminalId);
      return {
        projects,
        openTerminals: newOpenTerminals,
        selectedTerminalId: state.selectedTerminalId === terminalId
          ? (newOpenTerminals[newOpenTerminals.length - 1] ?? null)
          : state.selectedTerminalId,
      };
    });

    return { success: true };
  },

  updateServiceStatus: (projectName, serviceName, status, port) => set((state) => ({
    projects: state.projects.map((project) => {
      if (project.name !== projectName) return project;

      // Update service in project's direct services
      const updatedServices = project.services.map((service) => {
        if (service.name !== serviceName) return service;
        return { ...service, status, port: port ?? service.port };
      });

      // Also update in snapshots' services
      const updatedSnapshots = project.snapshots?.map((snapshot) => ({
        ...snapshot,
        services: snapshot.services.map((service) => {
          if (service.name !== serviceName) return service;
          return { ...service, status, port: port ?? service.port };
        }),
      }));

      return {
        ...project,
        services: updatedServices,
        snapshots: updatedSnapshots ?? [],
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

  // Settings actions
  initSettings: () => {
    const settings = loadSettings();
    set({
      settings,
      isSetupModalOpen: !settings.isFirstTimeSetupComplete,
    });
  },

  setSettings: (partial) => {
    const current = get().settings;
    const updated = { ...current, ...partial };
    saveSettings(updated);
    set({ settings: updated });
  },

  setSelectedAgents: (agents) => {
    const current = get().settings;
    const updated = { ...current, selectedAgents: agents };
    saveSettings(updated);
    set({ settings: updated });
  },

  completeFirstTimeSetup: () => {
    const current = get().settings;
    const updated = { ...current, isFirstTimeSetupComplete: true };
    saveSettings(updated);
    set({ settings: updated, isSetupModalOpen: false });
  },

  setSettingsModalOpen: (open) => set({ isSettingsModalOpen: open }),
  setSetupModalOpen: (open) => set({ isSetupModalOpen: open }),
  setCreateProjectModalOpen: (open) => set({ isCreateProjectModalOpen: open }),

  createProject: async (parentPath, projectName, agents) => {
    if (!window.envibe) {
      return { success: false, error: 'API not available' };
    }

    const result = await window.envibe.createProject(parentPath, projectName, agents);

    if ('error' in result) {
      return { success: false, error: result.error };
    }

    // Refresh the projects list
    const projects = await window.envibe.getProjects();
    set({ projects, isCreateProjectModalOpen: false });

    // Select the new project
    const newProject = projects.find((p) => p.path === result.path);
    if (newProject) {
      set({ selectedProjectName: newProject.name });
    }

    return { success: true };
  },
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
export const useSelectedProject = (): Project | null => {
  const projects = useStore((s) => s.projects);
  const selectedName = useStore((s) => s.selectedProjectName);
  return projects.find((p) => p.name === selectedName) ?? null;
};

export const useSelectedSnapshot = (): Snapshot | null => {
  const project = useSelectedProject();
  const selectedSnapshotId = useStore((s) => s.selectedSnapshotId);
  if (!project || !selectedSnapshotId) return null;
  return project.snapshots?.find((s) => s.id === selectedSnapshotId) ?? null;
};

export const useSelectedService = (): Service | null => {
  const project = useSelectedProject();
  const snapshot = useSelectedSnapshot();
  const selectedName = useStore((s) => s.selectedServiceName);

  // If a snapshot is selected, look in snapshot's services; otherwise project's services
  const services = snapshot?.services ?? project?.services ?? [];
  return services.find((s) => s.name === selectedName) ?? null;
};

// Get services for current context (snapshot or project root)
const EMPTY_SERVICES: Service[] = [];
export const useContextServices = (): Service[] => {
  const project = useSelectedProject();
  const snapshot = useSelectedSnapshot();
  return snapshot?.services ?? project?.services ?? EMPTY_SERVICES;
};

// Get terminals for current context (snapshot or project root)
const EMPTY_TERMINALS: TerminalSession[] = [];
export const useContextTerminals = (): TerminalSession[] => {
  const project = useSelectedProject();
  const snapshot = useSelectedSnapshot();
  return snapshot?.terminals ?? project?.terminals ?? EMPTY_TERMINALS;
};

// Check if a project is expanded in the tree view
export const useProjectExpanded = (projectName: string): boolean => {
  return useStore((s) => s.expandedProjects.has(projectName));
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
