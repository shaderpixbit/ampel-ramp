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

# Tests (Vitest — runs src/lib/ramp-utils.test.ts, 16 tests)
bun run test
bun run test:watch
bun run test -- -t "cycleRampStatus"   # run a single describe / it by name

# Production build (frontend + Rust bundle)
bun run tauri build
```

Tauri wraps the SvelteKit build: `beforeDevCommand = bun run dev`, `beforeBuildCommand = bun run build`. The Rust side is built by Cargo automatically via `tauri dev` / `tauri build`.

## Architecture

**Stack**: Tauri 2 (Rust backend) + SvelteKit 2 (static adapter) + Svelte 5 + TypeScript + Tailwind CSS 4.

### Data flow

State lives in a single **SQLite database** (`ampel.db`) next to the executable on the network drive. Uses **DELETE journal mode** (not WAL) with `busy_timeout=15000` and `synchronous=FULL` for safe concurrent access on SMB shares.

The frontend polls every **1.5 seconds** via a single `get_state(since_version)` invoke that returns `{ version, ramps, messages }`. When the client's `since_version` matches the server's `state_version`, `ramps` and `messages` come back as `null` so idle ticks skip both SELECTs and the JSON payload entirely. Mutations (`update_ramp`, `send_message`) bump the version; the client invalidates `lastVersion` to `null` after each mutation to force a resync on the next poll. There is no WebSocket or push — pull-based.

On first run, if the old JSON files (`ramps_state.json`, `chat_messages.json`, `ramp_events_*.json`) are present, they are automatically imported into SQLite and renamed to `*.bak`.

### Backend (`src-tauri/src/lib.rs`)

All Tauri commands live in a single `lib.rs`. The DB connection is held in `DbState(Mutex<Connection>)` and injected via Tauri's `State<DbState>`.

**Schema** — five tables:
- `ramps` — one row per ramp (id, name, status, last_updated_by, last_updated_at, locked_until, kennzeichen, notiz, reserviert_fuer)
- `chat_messages` — capped at 100 rows, last 50 shown
- `ramp_events` — append-only log written on every status change (indexed on `timestamp`)
- `users` — username → role mapping (roles: `admin`, `member_buero`, `member_lager`)
- `meta` — single row holding `state_version` (int counter); bumped by every `update_ramp` and `send_message` to drive the polling cursor

| Command | Description |
|---|---|
| `get_current_user` | Returns `USERNAME` / `USER` env var |
| `get_ramps` | Returns all ramps in canonical order; expired `locked_until` values are masked to `NULL` via `CASE WHEN` at SELECT time (no UPDATE — stale persisted values are harmless) |
| `get_state` | **Primary poll endpoint.** Takes `since_version`; returns full ramps+messages payload, or `null` for both when client's version matches |
| `update_ramp` | Writes event to `ramp_events` on status change, updates ramp row, sets 2-second lock on status change, bumps `state_version` |
| `get_messages` | Returns last 50 messages (oldest first) |
| `send_message` | Inserts message, trims table to 100 rows, bumps `state_version` |
| `get_daily_log` | Returns all `ramp_events` for today |
| `get_events_for_period` | Returns `ramp_events` for an arbitrary date range |
| `get_user_role` | Gets or creates role for a username (first user ever → admin; new users → member_lager) |
| `set_user_role` | Admin-only: sets role for any username |
| `get_all_users` | Returns all users sorted by username |

Ramps: **Section A** 42–30 (13 ramps), **Section B** 57–43 (15 ramps). Created via `INSERT OR IGNORE` on startup.

### Frontend (`src/routes/+page.svelte`)

This is the entire application UI — a single page. It uses **Svelte 5 runes** (`$state`, `$derived`, not stores or `$:`).

Key state:
- `ramps` / `messages` — synced via `get_state` every 1.5s
- `lastVersion: number | null` — version cursor sent to `get_state`; set to `null` after every mutation (cycleStatus / sendMessage / commitFieldEdit) to force a resync
- `now` — `$state(Date.now())` ticked every 1s by `clockInterval`; the dwell-timer and the lock-spinner read it directly for reactivity
- `userRole` — fetched once on mount via `get_user_role`; drives all permission checks. Updated locally by `changeUserRole` when an admin changes their own role
- `isUpdating` / `isSendingMessage` — per-operation flags (not shared)
- `isConnected` — set false on any `get_state` failure; only affects ramp buttons, not chat

Sections A and B are rendered as separate `<div>`s with `grid-template-columns: repeat(auto-fill, minmax(148px, 1fr))` (responsive, no fixed column count). Status colors use Troiber CSS variables: `--tr-green` = free, `--tr-warning` = pending, `--tr-red` = closed.

`src/lib/ramp-utils.ts` contains shared types (`Ramp`, `RampStatus`), `isRampLocked()`, `cycleRampStatus()`, `getStatusLabel()`, `formatTime()`, and `formatDate()` (which returns German `"Heute"` / `"Gestern"`). Unit tests live in `src/lib/ramp-utils.test.ts`.

`src/lib/components/ui/` contains a full shadcn/bits-ui component library that is **not currently used** by the main page — it's available for future UI work.

### User Role System

Three tiers control UI visibility and ramp interaction:

| Role | Status cycle on click | Editable fields | Modals |
|---|---|---|---|
| `admin` | Free ↔ Closed (direct toggle, skips Pending) | Kennzeichen + Reserviert-für | Protocol + User Management |
| `member_buero` | Free → Pending → Closed → Free (3-way) | Kennzeichen + Reserviert-für | Protocol only |
| `member_lager` | Free ↔ Closed (direct toggle) | Kennzeichen only | None |

New users default to `member_lager`. The first user ever inserted into the `users` table becomes `admin`. The `isBuero` derived state (`admin || member_buero`) gates the Protocol modal and the visibility of the Reserviert-für row. **Only `member_buero` reaches the Pending status via the click cycle** — admin and lager skip it via direct toggle. Pending can still appear on a ramp set by a buero user; everyone sees it.

## Key constraints

- Window is **1600×920, non-resizable** (`tauri.conf.json`). All layout decisions must fit this fixed size.
- No SSR — `@sveltejs/adapter-static` with `fallback: "index.html"`.
- Bun is the package manager (use `bun` not `npm`/`pnpm`).
- The 2-second per-ramp lock is enforced client-side via `locked_until` timestamp comparison and server-side by `update_ramp` setting a fresh `now+2000` only on status change.
- Inline field edits (Kennzeichen, Reserviert-für) do **not** change `last_updated_at` and do **not** set a lock — preserving the dwell timer.
- **Lock-spinner reactivity gotcha**: the per-tile `@const locked` inside the `rampTile` snippet must read the reactive `now` $state directly (`ramp.locked_until > now`), NOT call `isRampLocked(ramp)`. With Svelte 5's fine-grained reactivity, calling `isRampLocked` (which reads `Date.now()` non-reactively) means the spinner only re-evaluates when the `ramps` array reference changes — leaving the spinner stuck until the next poll-driven reassignment. Event handlers like `cycleStatus` can keep using `isRampLocked` because they evaluate at click time.
- `getStatusColor` and `formatDateTime` were removed as dead code; if a future change needs Tailwind class strings or a combined date/time formatter, write fresh helpers rather than restoring the old ones.
