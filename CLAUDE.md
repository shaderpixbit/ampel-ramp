# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Development (runs Tauri dev shell which starts Vite on port 1420)
bun run tauri dev

# Frontend-only (no Tauri window, browser at http://localhost:1420)
bun run dev

# Frontend type-check
bun run check
bun run check:watch   # watch mode

# Tests (Vitest — runs src/lib/ramp-utils.test.ts, ~31 tests)
bun run test
bun run test:watch

# Production build (frontend + Rust bundle)
bun run tauri build
```

Tauri wraps the SvelteKit build: `beforeDevCommand = bun run dev`, `beforeBuildCommand = bun run build`. The Rust side is built by Cargo automatically via `tauri dev` / `tauri build`.

## Architecture

**Stack**: Tauri 2 (Rust backend) + SvelteKit 2 (static adapter) + Svelte 5 + TypeScript + Tailwind CSS 4.

### Data flow

State lives in a single **SQLite database** (`ampel.db`) next to the executable on the network drive. Uses **DELETE journal mode** (not WAL) with `busy_timeout=15000` and `synchronous=FULL` for safe concurrent access on SMB shares.

The frontend polls every **1.5 seconds** via Tauri `invoke()` calls. There is no WebSocket or event push — everything is pull-based.

On first run, if the old JSON files (`ramps_state.json`, `chat_messages.json`, `ramp_events_*.json`) are present, they are automatically imported into SQLite and renamed to `*.bak`.

### Backend (`src-tauri/src/lib.rs`)

All Tauri commands live in a single `lib.rs`. The DB connection is held in `DbState(Mutex<Connection>)` and injected via Tauri's `State<DbState>`.

**Schema** — four tables:
- `ramps` — one row per ramp (id, name, status, last_updated_by, last_updated_at, locked_until, kennzeichen, notiz, reserviert_fuer)
- `chat_messages` — capped at 100 rows, last 50 shown
- `ramp_events` — append-only log written on every status change (indexed on `timestamp`)
- `users` — username → role mapping (roles: `admin`, `member_buero`, `member_lager`)

| Command | Description |
|---|---|
| `get_current_user` | Returns `USERNAME` / `USER` env var |
| `get_ramps` | Clears expired locks via SQL UPDATE, returns all ramps in canonical order |
| `update_ramp` | Writes event to `ramp_events`, updates ramp row, sets 2-second lock on status change |
| `get_messages` | Returns last 50 messages (oldest first) |
| `send_message` | Inserts message, trims table to 100 rows |
| `get_daily_log` | Returns all `ramp_events` for today |
| `get_events_for_period` | Returns `ramp_events` for an arbitrary date range |
| `get_user_role` | Gets or creates role for a username (first user ever → admin; new users → member_lager) |
| `set_user_role` | Admin-only: sets role for any username |
| `get_all_users` | Returns all users sorted by username |

Ramps: **Section A** 42–30 (13 ramps), **Section B** 57–43 (15 ramps). Created via `INSERT OR IGNORE` on startup.

### Frontend (`src/routes/+page.svelte`)

This is the entire application UI — a single page. It uses **Svelte 5 runes** (`$state`, `$derived`, not stores or `$:`).

Key state:
- `ramps` / `messages` — synced from backend every 1.5s
- `userRole` — fetched once on mount via `get_user_role`; drives all permission checks
- `isUpdating` / `isSendingMessage` — per-operation flags (not shared)
- `isConnected` — set false on any `get_ramps` failure; only affects ramp buttons, not chat

The 13-ramp grid is `grid-cols-13` — a non-standard Tailwind column count. Status colors use Troiber CSS variables: `--tr-green` = free, `--tr-warning` = pending, `--tr-red` = closed.

`src/lib/ramp-utils.ts` contains shared types (`Ramp`, `RampStatus`), `isRampLocked()`, `cycleRampStatus()`, `formatDateTime()`, and related helpers. Unit tests live in `src/lib/ramp-utils.test.ts`.

`src/lib/components/ui/` contains a full shadcn/bits-ui component library that is **not currently used** by the main page — it's available for future UI work.

### User Role System

Three tiers control UI visibility and ramp interaction:

| Role | Status cycle | Visible fields | Modals |
|---|---|---|---|
| `admin` | Free → Pending → Closed → Free | Kennzeichen + Reserviert-für | Protocol + User Management |
| `member_buero` | Free → Pending → Closed → Free | Kennzeichen + Reserviert-für | Protocol only |
| `member_lager` | Free ↔ Closed (skips Pending) | Kennzeichen only | None |

New users default to `member_lager`. The first user to ever open the app becomes `admin`. The `isBuero` derived state (`admin || member_buero`) gates the Protocol modal and the 3-way status cycle.

## Key constraints

- Window is **1600×920, non-resizable** (`tauri.conf.json`). All layout decisions must fit this fixed size.
- No SSR — `@sveltejs/adapter-static` with `fallback: "index.html"`.
- Bun is the package manager (use `bun` not `npm`/`pnpm`).
- The 2-second per-ramp lock is enforced client-side via `locked_until` timestamp comparison (`isRampLocked`) AND server-side by not overwriting an active lock in `update_ramp`.
- Inline field edits (Kennzeichen, Reserviert-für) do **not** change `last_updated_at` and do **not** set a lock — preserving the dwell timer.
