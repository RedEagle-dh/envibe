import { useEffect } from 'react';
import { useStore, useSelectedProject, useSelectedService, createLogEntry, parseLogLine } from './stores/useStore';
import { Sidebar } from './components/Sidebar';
import { ProjectsPanel } from './components/ProjectsPanel';
import { ServicesPanel } from './components/ServicesPanel';
import { LogViewer } from './components/LogViewer';
import { AgentTerminal, AgentTerminalDisconnected } from './components/AgentTerminal';
import { EnvPanel } from './components/EnvPanel';
import { Header } from './components/Header';
import type { Project, LogEntry } from './types';

// Process a single log line and return the log entry (or null for status updates)
function processLogLine(
  log: string,
  updateServiceStatus: (project: string, service: string, status: 'stopped' | 'running' | 'error') => void,
  setProjects: (projects: Project[]) => void
): LogEntry | null {
  const { level: parsedLevel, message } = parseLogLine(log);

  // Check for status update messages from the backend
  const statusMatch = message.match(/^\[__STATUS__\]\s*project=(\S+)\s+service=(\S+)\s+status=(\S+)/);
  if (statusMatch) {
    const [, project, service, status] = statusMatch;
    updateServiceStatus(project, service, status as 'stopped' | 'running' | 'error');
    window.envibe?.getProjects().then(setProjects);
    return null;
  }

  // Check for EXIT messages
  const exitMatch = message.match(/^\[([^\]\s]+)\s+EXIT\]\s*(.*)$/);
  if (exitMatch) {
    const serviceName = exitMatch[1];
    const exitMessage = exitMatch[2];
    return createLogEntry(exitMessage, { level: 'warn', service: serviceName });
  }

  // Parse service name from log if present
  const match = message.match(/^\[([^\]\s]+)(?:\s+ERR)?\]\s*(.*)$/);
  if (match) {
    const serviceName = match[1];
    const logMessage = match[2];
    const isError = message.includes(' ERR]');
    return createLogEntry(logMessage, {
      level: isError ? 'error' : parsedLevel,
      service: serviceName
    });
  }

  return createLogEntry(message, { level: parsedLevel });
}

export default function App() {
  const setProjects = useStore((s) => s.setProjects);
  const updateServiceStatus = useStore((s) => s.updateServiceStatus);
  const addLogs = useStore((s) => s.addLogs);
  const showEnvPanel = useStore((s) => s.showEnvPanel);
  const selectedProject = useSelectedProject();
  const selectedService = useSelectedService();

  const isAgentSelected = selectedService?.type === 'agent';
  const isAgentRunning = isAgentSelected && selectedService?.status === 'running';

  useEffect(() => {
    // Connect to Rust backend
    if (window.envibe) {
      // Throttled buffer: coalesce IPC batches into one store update per 150ms
      // 150ms is fast enough to feel real-time while reducing store updates from 60/sec to ~7/sec
      let pendingEntries: LogEntry[] = [];
      let flushTimer: ReturnType<typeof setTimeout> | null = null;

      const flushToStore = () => {
        if (pendingEntries.length > 0) {
          addLogs(pendingEntries.splice(0));
        }
        flushTimer = null;
      };

      const scheduleFlush = () => {
        if (flushTimer === null) {
          flushTimer = setTimeout(flushToStore, 150);
        }
      };

      // Handle batched logs (primary, more efficient)
      const unsubLogs = window.envibe.onLogs((logs) => {
        for (const log of logs) {
          const entry = processLogLine(log, updateServiceStatus, setProjects);
          if (entry) pendingEntries.push(entry);
        }
        scheduleFlush();
      });

      // Handle single logs (fallback for compatibility)
      const unsubLog = window.envibe.onLog((log) => {
        const entry = processLogLine(log, updateServiceStatus, setProjects);
        if (entry) pendingEntries.push(entry);
        scheduleFlush();
      });

      const unsubUpdate = window.envibe.onServiceUpdate((update) => {
        updateServiceStatus(update.project, update.service, update.status, update.port);
      });

      // Handle batched errors
      const unsubErrors = window.envibe.onRustErrors((errors) => {
        for (const e of errors) {
          pendingEntries.push(createLogEntry(e, { level: 'error' }));
        }
        scheduleFlush();
      });

      // Handle single errors (fallback)
      const unsubError = window.envibe.onRustError((error) => {
        pendingEntries.push(createLogEntry(error, { level: 'error' }));
        scheduleFlush();
      });

      // Load projects from backend
      window.envibe.getProjects().then((projects) => {
        setProjects(projects);
      });

      return () => {
        unsubLogs();
        unsubLog();
        unsubUpdate();
        unsubErrors();
        unsubError();
        if (flushTimer !== null) clearTimeout(flushTimer);
      };
    }
  }, [setProjects, addLogs, updateServiceStatus]);

  return (
    <div className="h-screen flex flex-col bg-envibe-bg overflow-hidden">
      <Header />
      <div className="flex-1 flex overflow-hidden">
        <Sidebar />
        <main className="flex-1 flex overflow-hidden p-4 gap-4">
          <div className="w-80 flex-shrink-0">
            <ProjectsPanel />
          </div>
          <div className="w-80 flex-shrink-0">
            <ServicesPanel />
          </div>
          <div className="flex-1 min-w-0">
            {isAgentRunning && selectedProject ? (
              <AgentTerminal
                projectName={selectedProject.name}
                serviceName={selectedService.name}
              />
            ) : isAgentSelected ? (
              <AgentTerminalDisconnected serviceName={selectedService!.name} />
            ) : (
              <LogViewer />
            )}
          </div>
          {showEnvPanel && (
            <div className="w-96 flex-shrink-0">
              <EnvPanel />
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
