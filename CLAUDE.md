# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Development (runs Tauri dev shell which starts Vite on port 1420)
bun run tauri dev

# Frontend type-check
bun run check
bun run check:watch   # watch mode

# Production build (frontend + Rust bundle)
bun run tauri build

# Frontend-only (no Tauri window)
bun run dev
```

Tauri wraps the SvelteKit build: `beforeDevCommand = bun run dev`, `beforeBuildCommand = bun run build`. The Rust side is built by Cargo automatically via `tauri dev` / `tauri build`.

## Architecture

**Stack**: Tauri 2 (Rust backend) + SvelteKit 2 (static adapter) + Svelte 5 + TypeScript + Tailwind CSS 4.

### Data flow

State lives in a single **SQLite database** (`ampel.db`) next to the executable on the network drive. WAL mode + `busy_timeout=5000` make concurrent access safe on SMB shares.

The frontend polls every **1.5 seconds** via Tauri `invoke()` calls. There is no WebSocket or event push — everything is pull-based.

On first run, if the old JSON files (`ramps_state.json`, `chat_messages.json`, `ramp_events_*.json`) are present, they are automatically imported into SQLite and renamed to `*.bak`.

### Backend (`src-tauri/src/lib.rs`)

All Tauri commands live in a single `lib.rs`. The DB connection is held in `DbState(Mutex<Connection>)` and injected via Tauri's `State<DbState>`.

**Schema** — three tables:
- `ramps` — one row per ramp (id, name, status, locked_until, kennzeichen, notiz, reserviert_fuer)
- `chat_messages` — capped at 100 rows, last 50 shown
- `ramp_events` — append-only log written on every status change (indexed on `timestamp`)

| Command | Description |
|---|---|
| `get_current_user` | Returns `USERNAME` / `USER` env var |
| `get_ramps` | Clears expired locks via SQL UPDATE, returns all ramps in canonical order |
| `update_ramp` | Writes event to `ramp_events`, updates ramp row, sets 2-second lock on status change |
| `get_messages` | Returns last 50 messages (oldest first) |
| `send_message` | Inserts message, trims table to 100 rows |
| `get_daily_log` | Returns all `ramp_events` for today |
| `get_events_for_period` | Returns `ramp_events` for an arbitrary date range |

Ramps: **Section A** 42–30 (13 ramps), **Section B** 57–43 (15 ramps). Created via `INSERT OR IGNORE` on startup.

### Frontend (`src/routes/+page.svelte`)

This is the entire application UI — a single page. It uses **Svelte 5 runes** (`$state`, `$derived`, not stores or `$:`).

Key state:
- `ramps` / `messages` — synced from backend every 1.5s
- `isUpdating` / `isSendingMessage` — per-operation flags (not shared)
- `isConnected` — set false on any `get_ramps` failure; only affects ramp buttons, not chat

The 13-ramp grid is `grid-cols-13` — a non-standard Tailwind column count. Status colors: emerald = free, amber = pending, rose = closed.

`src/lib/` contains a full shadcn/bits-ui component library that is **not currently used** by the main page — it's available for future UI work.

## Key constraints

- Window is **1600×920, non-resizable** (`tauri.conf.json`). All layout decisions must fit this fixed size.
- No SSR — `@sveltejs/adapter-static` with `fallback: "index.html"`.
- Bun is the package manager (use `bun` not `npm`/`pnpm`).
- The 3-second per-ramp lock is enforced client-side via `locked_until` timestamp comparison (`isRampLocked`) AND server-side by not overwriting an active lock in `update_ramp`.
