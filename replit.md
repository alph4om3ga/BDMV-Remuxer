# BDMV Remuxer

A Tauri desktop application that remuxes Blu-ray Disc Movie (BDMV) folders into organized MKV files with smart episode numbering, track management, and batch queue processing.

## Stack

- **Frontend:** React 18 + TypeScript (Vite)
- **Backend:** Rust (Tauri 1.6)
- **State:** Zustand
- **External deps:** MKVToolNix (`mkvmerge`) + FFmpeg (`ffprobe`) — must be installed on the host machine at runtime

## Project layout

The project currently has a flat structure — the two main source files live at the root:

- `App.tsx` — entire React frontend (~1300 lines)
- `main.rs` — entire Rust/Tauri backend (~700 lines)
- `package.json` / `Cargo.toml` — JS and Rust manifests

> **Note:** A typical Tauri project separates the frontend (`src/`) from the Tauri backend (`src-tauri/`), with `index.html`, `vite.config.ts`, and `tauri.conf.json` as required scaffolding files. Those are currently missing — the project needs this scaffolding added before it can be compiled or run via `npm run tauri dev`.

## Running / building

This is a **native desktop application** — it cannot be served as a web app on Replit. To build it locally:

1. Install Rust (rustup), Node.js 18+, MKVToolNix, and FFmpeg
2. Complete the Tauri project scaffolding (see Note above)
3. `npm install`
4. `npm run tauri dev` (development) or `npm run tauri build` (production binary)

## User preferences

<!-- Add any preferences here as they come up -->
