# Knative Explorer

A macOS menu-bar app for monitoring Knative services running on Kubernetes. Built with Tauri 2, React, and Rust.

## What

Knative Explorer lives in your menu bar and gives you a quick overview of your Knative services without switching to a terminal or dashboard. It shows service status, conditions, events, running pod counts, and lets you stream logs and ping endpoints — all from a single dropdown panel.

## Why

Knative's scale-to-zero model means pods come and go frequently. Checking service health typically requires `kubectl` commands or a full Kubernetes dashboard. This app provides a lightweight, always-accessible view of your Knative workloads — useful when developing or operating functions on a homelab or dev cluster.

## How

- **Rust backend** (`src-tauri/`) talks directly to the Kubernetes API via the `kube` crate. It reads your local kubeconfig, fetches Knative services/revisions/pods, resolves external URLs from Traefik IngressRoutes, and streams pod logs over Tauri's IPC channel.
- **React frontend** (`src/`) renders service cards with status indicators, conditions, events, and an inline log viewer with pause/resume support.
- **Kubernetes watchers** push live updates (pod count changes, service status) to the frontend via Tauri events so the UI stays current without polling.
- **Menu-bar mode** uses `tauri-plugin-positioner` to anchor the window to the tray icon. On macOS, the app runs as an Accessory (no dock icon).

## Features

- List Knative services by namespace
- View service conditions, latest revision, image tag, and recent events
- Live pod count via Kubernetes watch streams
- Stream pod logs in real-time with pause/resume and history separation
- Ping service endpoints with latency measurement
- Open service URLs in the browser
- System tray with click-to-toggle window

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (v18+) and [pnpm](https://pnpm.io/)
- [Tauri CLI](https://tauri.app/start/create-project/) (`pnpm add -g @tauri-apps/cli` or installed via devDependencies)
- A valid kubeconfig pointing at a cluster with Knative Serving installed
- macOS (primary target — other platforms may work but are untested)

## Build

```bash
# Install frontend dependencies
pnpm install

# Run in development mode (hot-reload frontend + Rust rebuilds)
pnpm tauri dev

# Build a release binary
pnpm tauri build
```

The release binary lands in `src-tauri/target/release/bundle/`.

## Project Structure

```
src/                    # React frontend
  components/           # UI components (ServiceCard, LogViewer, etc.)
  types.ts              # TypeScript types matching Rust structs
src-tauri/              # Rust backend
  src/
    lib.rs              # App setup, tray icon, state management
    commands.rs         # Tauri IPC command handlers
    logic.rs            # Kubernetes API calls and log streaming
    types.rs            # Domain types, serde definitions, CRD schemas
    watcher.rs          # Kubernetes watch streams for live updates
    error.rs            # Error types
```
