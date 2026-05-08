# Ramp Dashboard — User Guide

This guide explains how to use the Ramp Dashboard application during day-to-day yard operations. It is written for warehouse, dock and office staff using the app at the PC workstation.

> **Note on language:** the user interface is in German. This guide uses the original German labels (Frei / Wartend / Belegt etc.) and explains them in English. A German version of this document is available at [`USER_GUIDE_DE.md`](USER_GUIDE_DE.md).

## Contents

1. [At a glance](#at-a-glance)
2. [Status colors](#status-colors)
3. [Changing a ramp's status](#changing-a-ramps-status)
4. [Anatomy of a ramp tile](#anatomy-of-a-ramp-tile)
5. [License plates and reservations](#license-plates-and-reservations)
6. [The 2-second lock](#the-2-second-lock)
7. [Sync indicator](#sync-indicator)
8. [Team chat](#team-chat)
9. [Dark / Light mode](#dark--light-mode)
10. [Your username and role](#your-username-and-role)
11. [Protocol & statistics (office / admin)](#protocol--statistics-office--admin)
12. [User management (admin)](#user-management-admin)

---

## At a glance

When the app opens you see a window with three main areas:

- **Header bar** at the top — site name, sync indicator (`LIVE · 1.5s` / `OFFLINE`), status counts, **Protokoll** and **Benutzer** buttons, theme switch, your username and role.
- **Ramp overview** in the middle — **28 ramps** in two sectors:
  - **Sektor A · Tor 30–42** (13 ramps)
  - **Sektor B · Tor 43–57** (15 ramps)
- **Team chat** on the right — short messages to your colleagues.

![Full dashboard view](images/01-overview.png)

---

## Status colors

Every ramp has a status, shown by a color and an icon:

| Color | Meaning (German label) | Icon |
|-------|------------------------|------|
| **Green** | Free — ramp is empty and ready to use (`Frei`) | Check mark |
| **Amber** | Waiting — a truck is approaching or the ramp is reserved (`Wartend`) | Clock (pulsing) |
| **Red** | Occupied — a truck is docked, loading or unloading (`Belegt`) | Lock |

In the top header you'll find a summary chip per status showing how many ramps are currently in each state.

![Status counts summary](images/02-counts.png)

---

## Changing a ramp's status

A single **click** on a ramp tile cycles it to the next status. The cycle depends on your role:

| Role | Click behaviour |
|------|-----------------|
| **Admin** | `Frei` ↔ `Belegt` (direct toggle, skips `Wartend`) |
| **Büro** (office) | `Frei` → `Wartend` → `Belegt` → `Frei` (full 3-way cycle for reservations) |
| **Lager** (warehouse) | `Frei` ↔ `Belegt` (direct toggle, skips `Wartend`) |

The change becomes visible to all other staff in the warehouse within roughly 1.5 seconds.

![Cycling a ramp's status](images/03-status-cycle.png)

> **Tip:** Switching back to `Frei` automatically clears the license plate and reservation — the ramp is empty again.

---

## Anatomy of a ramp tile

Each ramp is a compact tile with three rows:

| Row | Content |
|-----|---------|
| **Top** | Gate number (e.g. `42`) and status badge (`FREI` / `WARTEND` / `BELEGT`) |
| **Middle** | **LKW** — license plate of the current truck (e.g. `M-TR 2418`) |
| **Bottom** | **Res.** — reservation note ("Reserviert für …") — visible to office/admin, and to anyone if a value is set |

Below those three rows a small info strip shows:

- **User and time** of the last status change
- **Dwell duration** in the current status (live counter, e.g. `02:14` for 2 min 14 s)

![Ramp tile anatomy](images/04-ramp-tile.png)

---

## License plates and reservations

**Lager** (warehouse) staff see and edit the license plate only.
**Büro / Admin** see and edit both the license plate and the reservation note.

To edit a field:

1. Click the license-plate field (`LKW`) or the reservation field (`Res.`) on the tile.
2. Type the value — for example `M-TR 2418` or `Lieferung Müller GmbH 14:30`.
3. Press **Enter** to save or **Esc** to cancel.

> **Important:** Inline edits do **not** change the timestamp of the last status change — the dwell counter keeps running while you fill in the license plate.

---

## The 2-second lock

When anyone changes a ramp's status, that ramp is locked for **2 seconds** across all clients. During the lock:

- A dark overlay with a spinning loader appears on the affected tile.
- Clicks are ignored.
- All other staff see the same lock at the same time.

This prevents two people from changing the same ramp simultaneously and creating conflicting state.

![Locked ramp](images/05-locked.png)

---

## Sync indicator

In the header there is a small badge:

- **Green dot + `LIVE · 1.5s`** — connection to the network drive is healthy. Everything stays in sync at a 1.5 s tick.
- **Dot + `OFFLINE`** — connection is lost. Data on screen may be out of date and clicks on ramps will not be saved.

![Sync and offline states](images/06-sync-indicator.png)

> **If you see `OFFLINE`:** check your network connection first. If the problem persists, contact your IT department. You usually don't need to restart the app — the badge automatically flips back to `LIVE` once the connection is re-established.

---

## Team chat

The right side of the window holds a simple chat panel for short coordination notes between yard staff.

- Type your message into the field at the bottom.
- Press **Enter** or click the send arrow.
- Your own messages appear on the right (dark bubble), messages from colleagues on the left (light bubble).
- A date separator (`Heute`, `Gestern`, or full date) is inserted whenever a new day begins.
- A small badge on the chat button shows the count of unread messages.

![Team chat](images/07-chat.png)

> **Note:** the panel shows the last 50 messages; up to 100 are kept in the database. Older messages are dropped automatically.

---

## Dark / Light mode

Top right, next to your username, you'll find a sun / moon switch.

- **Click the sun** → switch to dark mode.
- **Click the moon** → switch back to light mode.

Your choice is saved on your computer.

![Theme toggle](images/08-theme-toggle.png)

---

## Your username and role

The app automatically detects your Windows username and shows it in the top-right corner together with your role:

- **Admin** — full access including user management.
- **Büro** — protocol, license plate and reservation; full 3-way status cycle.
- **Lager** — default role for new users; license plate only, no protocol.

The name is recorded whenever you:

- change a ramp's status (visible in the info strip under the ramp), or
- send a chat message, or
- edit a license plate or reservation.

There is no separate login.

![User badge](images/09-user-badge.png)

> **First-time setup:** the very first user to open the app automatically becomes the admin. Every subsequent new user defaults to **Lager**. An admin can change roles afterwards (see [User management](#user-management-admin)).

---

## Protocol & statistics (office / admin)

Click **Protokoll** in the header to open a window with two parts:

- **Daily protocol** — chronological list of every status change today: time, ramp, before → after, dwell duration in the previous status, user, and license plate.
- **Statistics** — aggregate over a time range (day / week / month):
  - Number of status changes
  - Average dwell duration
  - Most active ramps
  - Most active staff

A **Print** button lets you save the protocol as PDF or send it to a printer.

![Protocol & statistics](images/10-protocol.png)

> **Note:** Lager users do not have access to this section.

---

## User management (admin)

Click **Benutzer** (visible to admins only) to open a table of all registered users with their current role.

For each user you can change the role:

- **Lager** (default) — status toggle and license plate only.
- **Büro** — additionally reservations and protocol.
- **Admin** — full access including user management.

Changes take effect immediately — affected users see the new role on the next sync tick (within ~1.5 s) without restarting the app.

![User management](images/11-user-mgmt.png)

> **Safety note:** always keep at least one admin in the system. If you demote yourself to Lager, you immediately lose access to user management.

---

## Screenshot capture checklist

Save all images under `docs/images/`:

- [ ] `01-overview.png` — full window with both sectors and a mix of states.
- [ ] `02-counts.png` — close-up of the three status chips (Frei / Wartend / Belegt) in the header.
- [ ] `03-status-cycle.png` — three ramp tiles side by side, one in each state.
- [ ] `04-ramp-tile.png` — single ramp tile with the three rows annotated (number/badge, LKW, Res., info strip).
- [ ] `05-locked.png` — a ramp immediately after a click, showing the loader overlay.
- [ ] `06-sync-indicator.png` — sync indicator in both states (a small composite is fine).
- [ ] `07-chat.png` — chat panel with a few sample messages and an unread-count badge.
- [ ] `08-theme-toggle.png` — the sun/moon switch, optionally two images (light / dark).
- [ ] `09-user-badge.png` — close-up of the user badge in the header showing role.
- [ ] `10-protocol.png` — protocol & statistics modal with sample data.
- [ ] `11-user-mgmt.png` — user management modal with several users and the role dropdown open.
