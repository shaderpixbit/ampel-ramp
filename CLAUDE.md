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

State lives in two JSON files on a **network drive**, written next to the executable:
- `ramps_state.json` — array of `Ramp` objects
- `chat_messages.json` — array of `ChatMessage` objects (capped at 200, last 50 shown)

The frontend polls both files every **1.5 seconds** via Tauri `invoke()` calls. There is no WebSocket or event push — everything is pull-based.

### Backend (`src-tauri/src/lib.rs`)

All Tauri commands live in a single `lib.rs`:

| Command | Description |
|---|---|
| `get_current_user` | Returns `USERNAME` / `USER` env var |
| `get_ramps` | Reads state file, clears expired locks, returns all 13 ramps |
| `update_ramp` | Validates status transition, sets a 3-second lock (`locked_until` = `now + 3000ms`) |
| `get_messages` | Returns last 50 messages |
| `send_message` | Appends message, trims to 200 total |

File access uses `fs4` for advisory exclusive locks (SMB/network drives may not honor them — the code warns and continues rather than failing).

Ramps are numbered **42 down to 30** (13 ramps total). On first run or missing state, `initialize_state()` creates them all as `"free"`.

### Frontend (`src/routes/+page.svelte`)

This is the entire application UI — a single page. It uses **Svelte 5 runes** (`$state`, `$derived`, not stores or `$:`).

Key state:
- `ramps` / `messages` — synced from backend every 1.5s
- `isUpdating` / `isSendingMessage` — per-operation flags (not shared)
- `isConnected` — set false on any `get_ramps` failure; only affects ramp buttons, not chat

The 13-ramp grid is `grid-cols-13` — a non-standard Tailwind column count. Status colors: emerald = free, amber = pending, rose = closed.

`src/lib/` contains a full shadcn/bits-ui component library that is **not currently used** by the main page — it's available for future UI work.

## Key constraints

- Window is **1600×800, non-resizable** (`tauri.conf.json`). All layout decisions must fit this fixed size.
- No SSR — `@sveltejs/adapter-static` with `fallback: "index.html"`.
- Bun is the package manager (use `bun` not `npm`/`pnpm`).
- The 3-second per-ramp lock is enforced client-side via `locked_until` timestamp comparison (`isRampLocked`) AND server-side by not overwriting an active lock in `update_ramp`.
