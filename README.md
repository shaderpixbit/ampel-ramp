# Ramp Dashboard

A desktop app for real-time management of 28 loading ramps across a local network. Built with Tauri 2 (Rust backend) and SvelteKit 2 / Svelte 5 (frontend). Used at the warehouse yard ("Werk Hofkirchen") for live coordination between dock and office staff.

## Features

- **28-ramp grid** in two sections — Sektor A (Tor 30–42, 13 ramps) and Sektor B (Tor 43–57, 15 ramps).
- **Three status states** per ramp — `Frei` (free), `Wartend` (waiting), `Belegt` (occupied).
- **Three user roles** with different permissions:
  - **Admin** — full access, user management, direct Free ↔ Closed toggle.
  - **Büro** — office staff, full 3-way cycle (Free → Pending → Closed → Free), can edit license plates and reservations, access protocol/statistics.
  - **Lager** — warehouse staff, direct Free ↔ Closed toggle, can edit license plates only.
- **Cross-client sync** — single SQLite database on a shared network drive, polled every 1.5 seconds via Tauri `invoke()`.
- **Version-tagged polling** — clients send their last seen version; the server returns an empty payload when nothing changed, so idle ticks skip the SELECT and JSON serialization entirely.
- **2-second lock** — every status change locks that ramp for 2 seconds across all clients to prevent collisions.
- **Inline editing** — license plate (`Kennzeichen`) and reservation note (`Reserviert für`) directly on each ramp tile.
- **Append-only protocol** — every status change is logged with timestamp, user, dwell duration, and license plate.
- **Statistics modal** — daily / weekly / monthly aggregates over the protocol log.
- **Team chat** — last 50 messages shown, capped at 100 stored.
- **Dark / Light mode** — persisted preference via `mode-watcher`.
- **User identity** — reads `USERNAME` / `USER` env var; the first user to ever open the app becomes admin, every subsequent user defaults to Lager.

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri 2 (Rust) |
| Database | SQLite (DELETE journal mode, `synchronous=FULL`, `busy_timeout=15000`) for safe concurrent access on SMB |
| Frontend framework | SvelteKit 2 · static adapter · SPA mode |
| UI language | Svelte 5 (runes: `$state`, `$derived`) |
| Styling | Tailwind CSS 4 · custom Troiber design tokens |
| Language | TypeScript |
| Package manager | Bun |
| Tests | Vitest 4 |

## Getting Started

```bash
# Install dependencies
bun install

# Start development (Tauri window + Vite HMR on :1420)
bun run tauri dev

# Frontend only (no Tauri window, browser at http://localhost:1420)
bun run dev
```

## Commands

```bash
bun run tauri dev      # Full Tauri dev shell
bun run dev            # Frontend only (Vite)
bun run tauri build    # Production build (frontend + Rust bundle)
bun run check          # SvelteKit type-check
bun run check:watch    # Type-check in watch mode
bun run test           # Run test suite (Vitest, single pass)
bun run test:watch     # Vitest in watch mode
```

## Architecture

### State — single SQLite file on the network drive

The database `ampel.db` lives next to the executable. WAL mode is intentionally avoided: WAL relies on shared-memory locking that SMB does not implement correctly under concurrent access. DELETE journal mode + a 15-second busy timeout are robust on SMB shares.

| Table | Purpose |
|---|---|
| `ramps` | One row per ramp (id, name, status, last_updated_by/at, locked_until, kennzeichen, notiz, reserviert_fuer) |
| `chat_messages` | Capped at 100; last 50 shown |
| `ramp_events` | Append-only protocol log (every status change), indexed on `timestamp` |
| `users` | username → role mapping (`admin` / `member_buero` / `member_lager`) |
| `meta` | Holds `state_version`, bumped on every mutation; used to skip polls when nothing changed |

On first run, legacy JSON files (`ramps_state.json`, `chat_messages.json`, `ramp_events_*.json`) are imported into SQLite and renamed to `*.bak`.

### Backend (`src-tauri/src/lib.rs`)

| Command | Description |
|---|---|
| `get_current_user` | Returns `USERNAME` / `USER` env var |
| `get_state` | Coalesced poll: takes `since_version`, returns `{ version, ramps, messages }` (ramps/messages `null` when unchanged) |
| `get_ramps` | Returns all ramps; expired locks are masked at SELECT time |
| `update_ramp` | Validates status, writes a `ramp_events` entry on changes, sets a 2-second lock |
| `get_messages` | Returns last 50 messages |
| `send_message` | Inserts message, trims to 100 total |
| `get_daily_log` | Returns today's `ramp_events` |
| `get_events_for_period` | Returns `ramp_events` for an arbitrary date range |
| `get_user_role` | Gets or creates the role for a username |
| `set_user_role` | Admin-only: sets the role for any user |
| `get_all_users` | Returns all users for the management modal |

### Frontend (`src/routes/+page.svelte`)

Single-page application using Svelte 5 runes (no stores, no `$:`). The polling loop calls `get_state` once every 1.5 s with the last seen `version`; when the server's version matches, the response is `{ version, ramps: null, messages: null }` and the client skips the assign.

Key UI characteristics:

- **Optimistic updates** — ramp status changes apply locally before the backend responds; rolled back on error.
- **2-second lock** — enforced both client-side via `locked_until` timestamp comparison and server-side by not overwriting an active lock.
- **Inline edits** for `Kennzeichen` and `Reserviert-für` do **not** change `last_updated_at` and do **not** set a lock — preserving the dwell timer.
- **Role-derived UI** — `userRole` drives every permission check. Updates immediately when an admin changes their own role.

### Utility module (`src/lib/ramp-utils.ts`)

Pure, framework-free functions shared between the page and the test suite:

- `isRampLocked(ramp)` — checks the `locked_until` timestamp (accepts a `Pick<Ramp, "locked_until">`).
- `cycleRampStatus(status)` — `Frei → Wartend → Belegt → Frei`.
- `getStatusLabel(status)` — German UI label.
- `formatTime(iso)` / `formatDate(iso)` — German-locale formatters; `formatDate` returns "Heute" / "Gestern" / `dd.MM.yyyy`.

## Testing

```bash
bun run test
```

16 unit tests cover the pure utility functions in `src/lib/ramp-utils.test.ts`. Tests run in the Node environment via Vitest — no browser or Tauri runtime required.

```
Test Files  1 passed (1)
     Tests  16 passed (16)
  Duration  ~150ms
```

## User Guide

End-user documentation (warehouse / dock / office staff) is in [`docs/USER_GUIDE_DE.md`](docs/USER_GUIDE_DE.md) and [`docs/USER_GUIDE_EN.md`](docs/USER_GUIDE_EN.md).

## Key Constraints

- Window is **1600 × 920, non-resizable** (`src-tauri/tauri.conf.json`).
- No SSR — `@sveltejs/adapter-static` with `fallback: "index.html"`.
- Tailwind dark mode uses the **class strategy**.
- Use `bun`, not `npm` or `pnpm`.
- The 13-column ramp grid uses a non-standard Tailwind `grid-cols-13`.

## Project Structure

```
src/
├── app.css                  # Global styles, Tailwind entry, Troiber CSS variables
├── lib/
│   ├── ramp-utils.ts        # Pure utility functions + shared types
│   ├── ramp-utils.test.ts   # Vitest unit tests
│   └── components/ui/       # shadcn/bits-ui library (available, not used by the main page)
└── routes/
    ├── +layout.svelte       # Mounts ModeWatcher, imports app.css
    ├── +layout.ts           # SSR disabled
    └── +page.svelte         # Entire application UI

src-tauri/
├── src/lib.rs               # All Tauri commands + DB schema + JSON migration
└── tauri.conf.json          # Window config (1600×920, non-resizable)

docs/
├── USER_GUIDE_DE.md         # German end-user guide
├── USER_GUIDE_EN.md         # English end-user guide
└── images/                  # Screenshots referenced by the guides

vitest.config.ts             # Test runner config
```
