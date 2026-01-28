# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Envibe is an Electron desktop application for orchestrating parallel development projects. It manages Docker containers, Docker Compose services, and local processes from a unified interface. The frontend is React/TypeScript with Vite, and it communicates with an external Rust backend via IPC.

## Development Commands

```bash
npm run dev              # Start Vite dev server + Electron in watch mode
npm run build            # Build for production (TypeScript + Vite + electron-builder)
npm run electron:watch   # Build Electron TypeScript and launch Electron
npm run build:rust       # Build Rust backend (in backend/)
```

The dev server runs on `http://localhost:5173`. DevTools open automatically in development.

**Important:** The dev server is always running and restarts automatically after changes. Never run `npm run dev` or `bun dev` - assume it's already running.

## Architecture

### Three-Layer Communication

1. **Rust Backend** (separate process on port 3847) - Manages Docker/services, provides REST API
2. **Electron Main Process** (`electron/main.ts`) - Spawns Rust backend, bridges IPC to frontend
3. **React Frontend** (`src/`) - UI with Zustand state management

```
React ←→ window.envibe (preload) ←→ IPC ←→ Electron Main ←→ HTTP ←→ Rust Backend
```

### Key Files

- `electron/main.ts` - App lifecycle, spawns Rust backend, IPC handlers for API calls
- `electron/preload.ts` - Exposes `window.envibe` API via context bridge (security boundary)
- `src/stores/useStore.ts` - Single Zustand store for all app state, includes selectors
- `src/types/index.ts` - TypeScript interfaces for Project, Service, LogEntry, etc.

### State Management Pattern

All state lives in a single Zustand store. Use provided selectors to prevent unnecessary re-renders:

```typescript
// Direct store access
const projects = useStore((s) => s.projects);

// Derived selectors
const project = useSelectedProject();  // Returns selected Project or null
const service = useSelectedService();  // Returns selected Service or null
const logs = useFilteredLogs();        // Returns logs for selected service
```

### IPC API

The `window.envibe` API exposed by preload:

- `getProjects()` / `addProject()` / `removeProject(projectPath)`
- `getServices(projectName)`
- `startService(project, service)` / `stopService(project, service)` / `restartService(project, service)`
- `setServicePort(project, service, port)`
- `getEnvVars(project, service?)` / `getBackendUrl()`
- `onLogs(callback)` / `onLog(callback)` - Batched and single log listeners
- `onServiceUpdate(callback)` / `onRustErrors(callback)` / `onRustError(callback)` - Returns unsubscribe function

### Log Processing Pipeline

Raw log lines from the Rust backend go through structured parsing in `App.tsx`:

1. Status updates detected via `[__STATUS__] project=... service=... status=...` pattern
2. Exit events detected via `[SERVICE EXIT]` pattern
3. Service name extracted from `[SERVICE_NAME]` prefix
4. Log level inferred from error/warn/debug keywords
5. Logs stored in Zustand (max 5000 entries, FIFO eviction)
6. Logs batched for IPC performance (flush at 100+ entries or next event loop tick)

Rust tracing output is filtered out automatically to only forward service output.

### Rust Backend

- Binary location: `backend/target/{debug,release}/envibe`
- Launched with: `envibe server --port 3847`
- Health check: `GET /health` (polled on startup, 30 attempts, 200ms interval)
- API endpoints: `/api/projects`, `/api/projects/:name/services`, `/api/services/{start,stop,restart}`, `/api/env/:project/:service?`

## TypeScript Configuration

Strict mode enabled with `noUnusedLocals` and `noUnusedParameters`. Path alias `@/*` maps to `src/*`.

Two separate configs:
- `tsconfig.json` - React frontend (noEmit, bundler resolution)
- `tsconfig.electron.json` - Electron main process (CommonJS, emits to dist-electron)

## Styling

Tailwind CSS with custom dark theme. Custom color tokens use the `envibe-` prefix (e.g., `bg-envibe-bg`, `text-envibe-accent`, `border-envibe-border`). Key custom classes defined in `index.css`:
- `.panel`, `.panel-header`, `.panel-content` - Container layout
- `.list-item` / `.list-item.selected` - List styling with hover/selection
- `.btn-primary`, `.btn-success`, `.btn-danger`, `.btn-ghost`, `.icon-btn` - Button variants
- `.badge-*`, `.status-dot-*` - Status indicators
- `.drag-region` / `.no-drag` - macOS window control regions

Font: JetBrains Mono with fallbacks to Fira Code/Monaco.

## Security Constraints

- Electron context isolation enabled, node integration disabled
- CSP restricts scripts to `'self'`, allows WebSocket connections to `127.0.0.1` and `localhost`
- Preload script is the only bridge between renderer and main process
- Environment panel masks sensitive values (keys containing secret/password/token/key)
