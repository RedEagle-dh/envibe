import { app, BrowserWindow, ipcMain, shell, dialog } from 'electron';
import * as path from 'path';
import { spawn, ChildProcess } from 'child_process';
import * as fs from 'fs';
import * as os from 'os';

let mainWindow: BrowserWindow | null = null;
let rustProcess: ChildProcess | null = null;

const isDev = !app.isPackaged;
// Use different ports for dev (3848) vs production (3847) to allow side-by-side testing
const RUST_API_PORT = isDev ? 3848 : 3847;
const RUST_API_URL = `http://127.0.0.1:${RUST_API_PORT}`;

function getRustBinaryPath(): string {
  const binaryName = os.platform() === 'win32' ? 'envibe.exe' : 'envibe';

  if (isDev) {
    // In development, use the debug or release build
    const releasePath = path.join(__dirname, '..', 'backend', 'target', 'release', binaryName);
    const debugPath = path.join(__dirname, '..', 'backend', 'target', 'debug', binaryName);

    if (fs.existsSync(releasePath)) return releasePath;
    if (fs.existsSync(debugPath)) return debugPath;

    console.warn('Rust binary not found. Running in frontend-only mode.');
    return '';
  }

  // In production, binary is bundled
  return path.join(process.resourcesPath, 'bin', binaryName);
}

async function waitForServer(maxAttempts = 30): Promise<boolean> {
  for (let i = 0; i < maxAttempts; i++) {
    try {
      const response = await fetch(`${RUST_API_URL}/health`);
      if (response.ok) {
        console.log('Rust server is ready');
        return true;
      }
    } catch {
      // Server not ready yet
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  return false;
}

function startRustBackend() {
  const binaryPath = getRustBinaryPath();
  if (!binaryPath) {
    console.log('Running without Rust backend');
    return;
  }

  console.log('Starting Rust backend:', binaryPath);

  rustProcess = spawn(binaryPath, ['server', '--port', String(RUST_API_PORT)], {
    stdio: ['pipe', 'pipe', 'pipe'],
  });

  // Buffer for batching logs
  let logBuffer: string[] = [];
  let errorBuffer: string[] = [];
  let flushHandle: ReturnType<typeof setImmediate> | null = null;

  // Filter out Rust tracing/internal logs (keep only service output)
  // Tracing logs look like: "2026-01-28T15:36:00.979844Z  INFO envibe::..."
  // or with ANSI codes: "\x1b[2m2026-01-28T15:36:00.979844Z\x1b[0m ..."
  const isRustTracingLog = (line: string): boolean => {
    // Remove ANSI escape codes for matching
    const clean = line.replace(/\x1b\[[0-9;]*m/g, '');
    // Match ISO timestamp at start followed by log level
    return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z\s+(INFO|DEBUG|WARN|ERROR|TRACE)\s+envibe::/.test(clean);
  };

  const flushLogs = () => {
    if (logBuffer.length > 0) {
      mainWindow?.webContents.send('rust-logs', logBuffer);
      logBuffer = [];
    }
    if (errorBuffer.length > 0) {
      mainWindow?.webContents.send('rust-errors', errorBuffer);
      errorBuffer = [];
    }
    flushHandle = null;
  };

  const scheduleFlush = () => {
    if (!flushHandle) {
      // Flush on next event loop iteration — batches all data from current tick
      // while avoiding the 16ms setTimeout delay
      flushHandle = setImmediate(flushLogs);
    }
  };

  rustProcess.stdout?.on('data', (data) => {
    const text = data.toString();
    const lines = text.split('\n').filter((line: string) => line.trim());
    for (const line of lines) {
      // Only send service logs to frontend, not internal Rust tracing logs
      if (!isRustTracingLog(line)) {
        logBuffer.push(line);
      }
    }
    // Flush immediately if buffer is large
    if (logBuffer.length >= 100) {
      if (flushHandle) {
        clearImmediate(flushHandle);
        flushHandle = null;
      }
      flushLogs();
    } else if (logBuffer.length > 0) {
      scheduleFlush();
    }
  });

  rustProcess.stderr?.on('data', (data) => {
    const text = data.toString();
    const lines = text.split('\n').filter((line: string) => line.trim());
    for (const line of lines) {
      errorBuffer.push(line);
    }
    if (errorBuffer.length >= 100) {
      if (flushHandle) {
        clearImmediate(flushHandle);
        flushHandle = null;
      }
      flushLogs();
    } else {
      scheduleFlush();
    }
  });

  rustProcess.on('close', (code) => {
    console.log(`Rust process exited with code ${code}`);
    if (code !== 0 && mainWindow) {
      mainWindow.webContents.send('rust-exit', code);
    }
    rustProcess = null;
  });

  rustProcess.on('error', (err) => {
    console.error('Failed to start Rust backend:', err);
  });

  // Wait for server to be ready
  waitForServer().then((ready) => {
    if (ready) {
      mainWindow?.webContents.send('rust-ready');
    }
  });
}

function stopRustBackend() {
  if (rustProcess) {
    rustProcess.kill('SIGTERM');
    rustProcess = null;
  }
}

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    minWidth: 1000,
    minHeight: 700,
    backgroundColor: '#0d1117',
    titleBarStyle: 'hiddenInset',
    trafficLightPosition: { x: 16, y: 16 },
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  if (isDev) {
    mainWindow.loadURL('http://localhost:5174'); // Dev port
    mainWindow.webContents.openDevTools();
  } else {
    mainWindow.loadFile(path.join(__dirname, '../dist/index.html'));
  }

  mainWindow.on('closed', () => {
    mainWindow = null;
  });

  // Open external links in browser
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    shell.openExternal(url);
    return { action: 'deny' };
  });
}

app.whenReady().then(() => {
  createWindow();
  startRustBackend();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  stopRustBackend();
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('before-quit', () => {
  stopRustBackend();
});

// Helper for API calls that return JSON
async function apiCall<T>(endpoint: string, options?: RequestInit): Promise<T | null> {
  try {
    const response = await fetch(`${RUST_API_URL}${endpoint}`, options);
    if (response.ok) {
      const text = await response.text();
      if (text) {
        return JSON.parse(text);
      }
    }
  } catch (error) {
    console.error(`API call failed: ${endpoint}`, error);
  }
  return null;
}

// Helper for API calls that don't return data
async function apiPost(endpoint: string, body: object): Promise<boolean> {
  try {
    const response = await fetch(`${RUST_API_URL}${endpoint}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    return response.ok;
  } catch (error) {
    console.error(`API call failed: ${endpoint}`, error);
    return false;
  }
}

// IPC handlers for communication with renderer
ipcMain.handle('get-projects', async () => {
  return await apiCall('/api/projects') ?? [];
});

ipcMain.handle('get-services', async (_event, projectName: string) => {
  return await apiCall(`/api/projects/${encodeURIComponent(projectName)}/services`) ?? [];
});

ipcMain.handle('start-service', async (_event, projectName: string, serviceName: string) => {
  return await apiPost('/api/services/start', { project: projectName, service: serviceName });
});

ipcMain.handle('stop-service', async (_event, projectName: string, serviceName: string) => {
  return await apiPost('/api/services/stop', { project: projectName, service: serviceName });
});

ipcMain.handle('restart-service', async (_event, projectName: string, serviceName: string) => {
  return await apiPost('/api/services/restart', { project: projectName, service: serviceName });
});

ipcMain.handle('set-service-port', async (_event, projectName: string, serviceName: string, port: number) => {
  return await apiCall('/api/services/port', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ project: projectName, service: serviceName, port }),
  });
});

ipcMain.handle('get-env-vars', async (_event, projectName: string, serviceName?: string) => {
  if (serviceName) {
    return await apiCall(`/api/env/${encodeURIComponent(projectName)}/${encodeURIComponent(serviceName)}`) ?? {};
  }
  return await apiCall(`/api/env/${encodeURIComponent(projectName)}`) ?? {};
});

ipcMain.handle('add-project', async () => {
  if (!mainWindow) return null;
  const result = await dialog.showOpenDialog(mainWindow, {
    properties: ['openDirectory'],
    title: 'Select Project Directory',
  });
  if (result.canceled || result.filePaths.length === 0) return null;

  const projectPath = result.filePaths[0];
  return await apiCall('/api/projects/add', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path: projectPath }),
  });
});

ipcMain.handle('get-backend-url', () => {
  return RUST_API_URL;
});

ipcMain.handle('remove-project', async (_event, projectPath: string) => {
  return await apiCall('/api/projects/remove', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path: projectPath }),
  });
});

ipcMain.handle('select-directory', async (_event, title?: string) => {
  if (!mainWindow) return null;
  const result = await dialog.showOpenDialog(mainWindow, {
    properties: ['openDirectory', 'createDirectory'],
    title: title ?? 'Select Directory',
  });
  if (result.canceled || result.filePaths.length === 0) return null;
  return result.filePaths[0];
});

ipcMain.handle('create-project', async (_event, parentPath: string, projectName: string, agents: string[]) => {
  // Validate inputs
  if (!parentPath || !projectName) {
    return { error: 'Parent path and project name are required' };
  }

  // Sanitize project name (remove invalid chars for filesystem)
  const safeName = projectName.replace(/[<>:"/\\|?*]/g, '-').trim();
  if (!safeName) {
    return { error: 'Invalid project name' };
  }

  const projectPath = path.join(parentPath, safeName);

  // Check if directory already exists
  if (fs.existsSync(projectPath)) {
    return { error: 'A folder with this name already exists' };
  }

  try {
    // Create the project directory
    fs.mkdirSync(projectPath, { recursive: true });

    // Build the .envibe.yaml content (agents parameter is now ignored - agents are created on-demand)
    const yamlContent = `name: ${safeName}\n\nservices: {}\n`;

    // Write the .envibe.yaml file
    fs.writeFileSync(path.join(projectPath, '.envibe.yaml'), yamlContent, 'utf-8');

    // Add the project to the registry via API
    const addResult = await apiCall('/api/projects/add', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path: projectPath }),
    });

    if (!addResult) {
      return { error: 'Failed to add project to registry' };
    }

    return { status: 'created', path: projectPath };
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Unknown error';
    return { error: `Failed to create project: ${message}` };
  }
});

// Snapshot management
ipcMain.handle('create-snapshot', async (_event, projectName: string, name: string, branch: string) => {
  return await apiCall('/api/snapshots/create', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ project: projectName, name, branch }),
  }) ?? { error: 'Failed to create snapshot' };
});

ipcMain.handle('delete-snapshot', async (_event, projectName: string, snapshotId: string) => {
  return await apiCall('/api/snapshots/delete', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ project: projectName, snapshotId }),
  }) ?? { error: 'Failed to delete snapshot' };
});

ipcMain.handle('merge-snapshot', async (_event, projectName: string, snapshotId: string, deleteAfterMerge: boolean, commitMessage?: string) => {
  // Merge can return non-200 status codes (400 for uncommitted changes, 409 for conflicts)
  // but still includes useful error info in the body, so we need custom handling
  try {
    const response = await fetch(`${RUST_API_URL}/api/snapshots/merge`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ project: projectName, snapshotId, deleteAfterMerge, commitMessage }),
    });
    const text = await response.text();
    if (text) {
      return JSON.parse(text);
    }
    return { success: false, message: 'Empty response from server', hasConflicts: false, conflictFiles: [] };
  } catch (error) {
    console.error('Merge snapshot failed:', error);
    return { success: false, message: 'Failed to merge snapshot', hasConflicts: false, conflictFiles: [] };
  }
});

// Terminal management
ipcMain.handle('create-terminal', async (_event, projectName: string, snapshotId?: string, terminalType?: string, agentCommand?: string) => {
  return await apiCall('/api/terminals/create', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ project: projectName, snapshotId, terminalType, agentCommand }),
  }) ?? { error: 'Failed to create terminal' };
});

ipcMain.handle('close-terminal', async (_event, terminalId: string) => {
  return await apiCall('/api/terminals/close', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ terminalId }),
  }) ?? { error: 'Failed to close terminal' };
});
