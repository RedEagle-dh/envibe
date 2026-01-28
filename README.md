# Envibe

A desktop application for orchestrating parallel development projects. Manage Docker containers, Docker Compose services, local processes, and AI agents from a single interface.

Built with Electron, React, and a Rust backend.

## Features

- **Project management** -- Add local project directories and auto-discover their services (Docker Compose, standalone containers, processes)
- **Service control** -- Start, stop, and restart individual services or all at once
- **Unified log viewer** -- Aggregated, color-coded logs with filtering, search, auto-follow, and copy/clear controls
- **Agent terminal** -- Interactive xterm.js terminal for AI agent services via WebSocket
- **Environment inspector** -- View environment variables grouped by source with sensitive value masking
- **Port management** -- Assign and track ports across services

## Prerequisites

- [Node.js](https://nodejs.org/) (LTS recommended)
- [Rust toolchain](https://rustup.rs/) (for building the backend)
- [Docker](https://www.docker.com/) (for container-based services)

## Installation

### Quick install (build + install)

```bash
git clone <repo-url> && cd envibe
./install.sh        # Build everything (Rust backend + Electron app)
./setup.sh          # Install for current user
```

After setup, launch from your app launcher (SUPER key, search "Envibe") or run `envibe` from a terminal.

### Build options

`install.sh` builds the Rust backend, compiles the frontend, and packages the Electron app.

```bash
./install.sh                  # Unpacked build (fast, default)
./install.sh AppImage         # AppImage (universal Linux binary)
./install.sh tar.gz           # Compressed archive
./install.sh --all            # All formats (AppImage, tar.gz, deb, rpm, pacman)
./install.sh --help           # Full usage info
```

Packaged formats (AppImage, tar.gz, deb, rpm, pacman) compress ~200MB of Electron + Chromium and take a few minutes. The default `dir` target produces an unpacked build instantly.

### Setup options

`setup.sh` installs the unpacked build to your system and creates a desktop entry for app launchers.

```bash
./setup.sh                    # User-local install (no sudo)
./setup.sh --system           # System-wide install to /opt (sudo)
./setup.sh uninstall          # Remove user-local install
./setup.sh uninstall --system # Remove system-wide install
./setup.sh --help             # Full usage info
```

| | User-local (default) | System-wide (`--system`) |
|---|---|---|
| App files | `~/.local/share/envibe/` | `/opt/envibe/` |
| Binary | `~/.local/bin/envibe` | `/usr/local/bin/envibe` |
| Desktop entry | `~/.local/share/applications/envibe.desktop` | `/usr/share/applications/envibe.desktop` |

## Development

```bash
npm install
npm run build:rust   # Build the Rust backend (first time only)
npm run dev          # Start Vite dev server + Electron
```

The dev server runs on `http://localhost:5173`. The Rust backend starts on port `3847`. DevTools open automatically in development.

### Scripts

| Command | Description |
|---|---|
| `npm run dev` | Start Vite dev server + Electron in watch mode |
| `npm run build` | Production build (TypeScript + Vite + electron-builder) |
| `npm run build:rust` | Build Rust backend in release mode |
| `npm run electron:watch` | Compile Electron TypeScript and launch Electron |

## Architecture

```
React Frontend <-> window.envibe (preload) <-> IPC <-> Electron Main <-> HTTP <-> Rust Backend
```

| Layer | Role |
|---|---|
| **Rust Backend** (`backend/`, port 3847) | Manages Docker and local processes, exposes a REST + WebSocket API |
| **Electron Main** (`electron/main.ts`) | Spawns the Rust backend, bridges IPC between frontend and backend |
| **React Frontend** (`src/`) | UI with Zustand state management, communicates through the preload context bridge |

The Electron preload script (`electron/preload.ts`) is the security boundary -- context isolation is enabled and node integration is disabled. The frontend can only access the Rust backend through the exposed `window.envibe` API.

## Project Structure

```
backend/                 # Rust backend (Axum HTTP server, Docker/process management)
  Cargo.toml
  src/
    main.rs             # CLI entry point (server + TUI modes)
    server/             # HTTP/WebSocket server for Electron
    docker/             # Docker interaction via Bollard
    process/            # Local process management
    agent/              # AI agent terminal handling
    config/             # YAML configuration parsing
electron/
  main.ts              # App lifecycle, Rust backend spawn, IPC handlers
  preload.ts           # Context bridge (window.envibe API)
src/
  components/          # React UI components
    AgentTerminal.tsx   # xterm.js WebSocket terminal for agents
    EnvPanel.tsx        # Environment variable viewer
    Header.tsx          # Top bar with controls
    LogViewer.tsx       # Virtualized log console
    ProjectsPanel.tsx   # Project list sidebar
    ServicesPanel.tsx   # Service list with start/stop controls
    Sidebar.tsx         # Left nav with quick actions
  stores/
    useStore.ts         # Zustand store (all app state + selectors)
  types/
    index.ts            # TypeScript interfaces
  App.tsx               # Root component, IPC listener wiring, layout
```

## Tech Stack

| Category | Technology |
|---|---|
| Desktop shell | Electron 40 |
| Frontend | React 19, TypeScript 5.9 |
| Bundler | Vite 7 |
| State | Zustand 5 |
| Styling | Tailwind CSS 3 (custom dark theme) |
| Icons | Lucide React |
| Terminal | xterm.js 6 |
| Backend | Rust (Axum, Bollard, Tokio) |

## License

MIT
