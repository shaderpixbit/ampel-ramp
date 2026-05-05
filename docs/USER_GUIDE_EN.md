# Ramp Dashboard — User Guide

This guide explains how to use the Ramp Dashboard application during day-to-day yard operations. It is written for warehouse and dock-yard staff using the app at the PC workstation.

> **Note on language:** the user interface is in German. This guide uses the original German labels (Frei / Wartend / Belegt etc.) and explains them in English. A German version of this document is available at [`USER_GUIDE_DE.md`](USER_GUIDE_DE.md).

## Contents

1. [At a glance](#at-a-glance)
2. [Status colors](#status-colors)
3. [Changing a ramp's status](#changing-a-ramps-status)
4. [The truck graphic](#the-truck-graphic)
5. [The 3-second lock](#the-3-second-lock)
6. [Sync indicator](#sync-indicator)
7. [Team chat](#team-chat)
8. [Dark / Light mode](#dark--light-mode)
9. [Your username](#your-username)

---

## At a glance

When the app opens you see a window with three main areas:

- **Header bar** at the top — title, sync indicator, theme switch, and your username.
- **Ramp overview** in the middle — the 13 ramps (numbered 42 down to 30) shown as a top-down loading-yard view.
- **Team chat** on the right — short messages to your colleagues.

![Full dashboard view](images/01-overview.png)

---

## Status colors

Every ramp has a status, shown by a color and an icon:

| Color | Meaning (German label) | Icon |
|-------|------------------------|------|
| **Green** | Free — ramp is empty and ready to use (`Frei`) | Check mark |
| **Amber** | Waiting — a truck is approaching or maneuvering (`Wartend`) | Clock |
| **Red** | Occupied — a truck is docked, loading or unloading (`Belegt`) | Lock |

In the top-right corner of the ramp section you'll find a summary showing how many ramps are currently in each state.

![Status counts summary](images/02-counts.png)

---

## Changing a ramp's status

A single **click** on a ramp tile cycles it to the next status:

```
Frei (Free) → Wartend (Waiting) → Belegt (Occupied) → Frei → ...
```

The change becomes visible to all other staff in the warehouse within roughly 1.5 seconds.

![Cycling a ramp's status](images/03-status-cycle.png)

> **Tip:** Hover the mouse over a ramp tile to see the lift animation — that's a quick visual confirmation the tile is clickable.

---

## The truck graphic

Each ramp tile is drawn as a small top-down view of the dock bay:

- **The grey wall at the top** represents the warehouse building, with the ramp number printed on it.
- **The colored strip** is the dock door (in the current status color).
- **The area below** is the apron — the asphalt where the truck approaches and parks.
- **The truck** (white trailer + dark cab) appears as soon as the ramp is no longer free.

| Status | Where is the truck? |
|--------|---------------------|
| **Free** | No truck — the apron is empty. |
| **Waiting** | Truck is parked further down the apron and pulses gently — it is approaching the dock. |
| **Occupied** | Truck is fully docked at the door, settled and stationary. |

![Truck in the three states](images/04-truck-states.png)

---

## The 3-second lock

When anyone changes a ramp's status, that ramp is locked for **3 seconds** across all clients. During the lock:

- A dark overlay with a spinning loader appears on the affected tile.
- Clicks are ignored.
- All other staff see the same lock at the same time.

This prevents two people from changing the same ramp simultaneously and creating conflicting state.

![Locked ramp](images/05-locked.png)

---

## Sync indicator

Next to the title in the header there is a small badge:

- **Green dot + "Sync"** — connection to the network drive is healthy. Everything stays in sync.
- **Red dot + "Offline"** — connection is lost. Data on screen may be out of date and clicks on ramps will not be saved.

![Sync and offline states](images/06-sync-indicator.png)

> **If you see "Offline":** check your network connection first. If the problem persists, contact your IT department. You usually don't need to restart the app — the badge automatically flips back to "Sync" once the connection is re-established.

---

## Team chat

The right side of the window holds a simple chat panel for short coordination notes between yard staff.

- Type your message into the field at the bottom.
- Press **Enter** or click the send arrow.
- Your own messages appear on the right (dark bubble), messages from colleagues on the left (light bubble).
- A date separator is inserted whenever a new day begins.

![Team chat](images/07-chat.png)

> **Note:** the panel shows the last 50 messages; up to 200 are kept on the network drive. Older messages are dropped automatically.

---

## Dark / Light mode

Top right, next to your username, you'll find a sun / moon switch.

- **Click the sun** → switch to dark mode.
- **Click the moon** → switch back to light mode.

Your choice is saved on your computer.

![Theme toggle](images/08-theme-toggle.png)

---

## Your username

The app automatically detects your Windows username and shows it in the top-right corner. This name is recorded whenever you:

- change a ramp's status (visible in the small info strip under the ramp), or
- send a chat message.

There is no separate login.

![User badge](images/09-user-badge.png)

---

## Screenshot capture checklist

Save all images under `docs/images/`:

- [ ] `01-overview.png` — full window with ramps in mixed states.
- [ ] `02-counts.png` — close-up of the three status chips (Frei / Wartend / Belegt) at the top of the ramp section.
- [ ] `03-status-cycle.png` — three ramps side by side, one in each state.
- [ ] `04-truck-states.png` — three ramps showing the truck in empty / waiting / docked positions.
- [ ] `05-locked.png` — a ramp immediately after a click, showing the loader overlay.
- [ ] `06-sync-indicator.png` — the sync indicator in both states (a small composite is fine).
- [ ] `07-chat.png` — chat panel with a few sample messages from different people.
- [ ] `08-theme-toggle.png` — the sun/moon switch, optionally two images (light / dark).
- [ ] `09-user-badge.png` — close-up of the user badge in the header.
