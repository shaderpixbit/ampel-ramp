<script lang="ts">
    import { onMount, onDestroy, tick } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { toggleMode } from "mode-watcher";
    import {
        isRampLocked,
        getStatusLabel,
        cycleRampStatus,
        formatTime,
        formatDate,
        type Ramp,
        type RampStatus,
    } from "$lib/ramp-utils";
    import { LoaderCircleIcon } from "@lucide/svelte";

    interface ChatMessage {
        id: string;
        user: string;
        text: string;
        timestamp: string;
    }

    interface StatePayload {
        version: number;
        ramps: Ramp[] | null;
        messages: ChatMessage[] | null;
    }

    let ramps = $state<Ramp[]>([]);
    let currentUser = $state("Unknown User");
    let syncTimeout: number;
    let isUpdating = $state(false);
    let isConnected = $state(true);
    let isFetching = false;
    let lastVersion: number | null = null;
    let syncError = $state<string | null>(null);
    let errorTimeout: number;

    let messages = $state<ChatMessage[]>([]);
    let chatInput = $state("");
    let isSendingMessage = $state(false);
    let chatScrollEl: HTMLElement;
    let chatOpen = $state(false);
    let lastReadCount = $state(0);
    let unreadCount = $derived(
        chatOpen ? 0 : Math.max(0, messages.length - lastReadCount),
    );

    let now = $state(Date.now());
    let clockInterval: number;

    let counts = $derived({
        free: ramps.filter((r) => r.status === "free").length,
        pending: ramps.filter((r) => r.status === "pending").length,
        closed: ramps.filter((r) => r.status === "closed").length,
    });

    function showError(message: string) {
        syncError = message;
        clearTimeout(errorTimeout);
        errorTimeout = window.setTimeout(() => {
            syncError = null;
        }, 4000);
    }

    onMount(async () => {
        try {
            currentUser = await invoke("get_current_user");
        } catch (e) {
            console.error("Failed to get current user:", e);
        }
        try {
            userRole = await invoke("get_user_role", { username: currentUser });
        } catch (e) {
            console.error("Failed to get user role:", e);
        }

        await fetchRamps();

        try {
            messages = await invoke("get_messages");
            lastReadCount = messages.length;
        } catch (e) {
            console.error("Failed to load messages:", e);
        }

        clockInterval = window.setInterval(() => {
            now = Date.now();
        }, 1000);
        await fetchDailyLog();
        syncTimeout = window.setTimeout(startPolling, 1500);
    });

    onDestroy(() => {
        clearTimeout(syncTimeout);
        clearTimeout(errorTimeout);
        clearInterval(clockInterval);
    });

    async function startPolling() {
        if (!isUpdating && !isFetching) {
            isFetching = true;
            try {
                const payload = await invoke<StatePayload>("get_state", {
                    sinceVersion: lastVersion,
                });

                if (payload.ramps !== null) ramps = payload.ramps;

                if (payload.messages !== null) {
                    messages = payload.messages;
                    if (chatOpen) {
                        lastReadCount = messages.length;
                        await tick();
                        if (chatScrollEl)
                            chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
                    }
                }

                lastVersion = payload.version;
                isConnected = true;
            } catch (e) {
                console.error("Failed to fetch state from network drive:", e);
                isConnected = false;
            } finally {
                isFetching = false;
            }
        }
        syncTimeout = window.setTimeout(startPolling, 1500);
    }

    async function fetchRamps() {
        try {
            const dbRamps: Ramp[] = await invoke("get_ramps");
            ramps = dbRamps;
            isConnected = true;
        } catch (e) {
            console.error("Failed to fetch state from network drive:", e);
            isConnected = false;
            showError("Verbindung zum Netzlaufwerk fehlgeschlagen.");
        }
    }

    async function cycleStatus(ramp: Ramp) {
        if (isUpdating || isRampLocked(ramp)) return;
        // admin + member_buero: free → pending → closed → free  (full 3-way cycle for reservations)
        // member_lager: free/pending → closed → free  (direct toggle)
        const newStatus = isBuero
            ? cycleRampStatus(ramp.status)
            : ramp.status === "closed"
              ? "free"
              : "closed";
        // Büro/Admin going free → pending: prompt for LKW + Reservierung first
        if (isBuero && ramp.status === "free" && newStatus === "pending") {
            pendingDialog = {
                ramp,
                kennzeichen: ramp.kennzeichen ?? "",
                reserviert_fuer: ramp.reserviert_fuer ?? "",
            };
            return;
        }
        await applyStatusChange(ramp, newStatus);
    }

    async function applyStatusChange(
        ramp: Ramp,
        newStatus: RampStatus,
        fields: Partial<
            Pick<Ramp, "kennzeichen" | "reserviert_fuer" | "notiz">
        > = {},
    ) {
        const now_iso = new Date().toISOString();
        const updatedRamp: Ramp = {
            ...ramp,
            ...fields,
            status: newStatus,
            last_updated_by: currentUser,
            last_updated_at: now_iso,
            ...(newStatus === "free" && {
                kennzeichen: null,
                notiz: null,
                reserviert_fuer: null,
            }),
        };
        const index = ramps.findIndex((r) => r.id === ramp.id);
        const oldRamp = index !== -1 ? { ...ramps[index] } : null;
        if (index !== -1) ramps[index] = updatedRamp;
        isUpdating = true;
        try {
            const updatedRamps: Ramp[] = await invoke("update_ramp", {
                updatedRamp,
            });
            ramps = updatedRamps;
            lastVersion = null; // resync version cursor on next poll
            fetchDailyLog(); // refresh log after every status change
        } catch (e) {
            console.error("Failed to update ramp:", e);
            if (index !== -1 && oldRamp) ramps[index] = oldRamp;
            showError(`Rampe ${ramp.id} konnte nicht aktualisiert werden.`);
        } finally {
            isUpdating = false;
        }
        releaseFocus();
    }

    async function confirmPendingDialog() {
        if (!pendingDialog) return;
        const { ramp, kennzeichen, reserviert_fuer } = pendingDialog;
        pendingDialog = null;
        await applyStatusChange(ramp, "pending", {
            kennzeichen: kennzeichen.trim() || null,
            reserviert_fuer: reserviert_fuer.trim() || null,
        });
    }

    function cancelPendingDialog() {
        pendingDialog = null;
        releaseFocus();
    }

    async function sendMessage() {
        const text = chatInput.trim();
        if (!text || isSendingMessage) return;
        chatInput = "";
        isSendingMessage = true;
        try {
            const updated: ChatMessage[] = await invoke("send_message", {
                user: currentUser,
                text,
            });
            messages = updated;
            lastVersion = null; // resync version cursor on next poll
            lastReadCount = messages.length;
            await tick();
            if (chatScrollEl)
                chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
        } catch (e) {
            console.error("Failed to send message:", e);
            chatInput = text;
            showError("Nachricht konnte nicht gesendet werden.");
        } finally {
            isSendingMessage = false;
        }
    }

    async function openChat() {
        chatOpen = true;
        lastReadCount = messages.length;
        await tick();
        if (chatScrollEl) chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
    }

    function closeChat() {
        chatOpen = false;
    }

    function parseTsMs(ts: string): number {
        const ms = new Date(ts).getTime();
        if (!isNaN(ms)) return ms;
        // fallback for "YYYY-MM-DD HH:MM:SS" (backend init format)
        return new Date(ts.replace(" ", "T")).getTime();
    }

    function getStatusTone(status: string) {
        if (status === "free")
            return {
                fg: "var(--tr-green)",
                bg: "var(--tr-green-bg)",
                line: "var(--tr-green-line)",
                glow: "var(--tr-green-glow)",
                label: "Frei",
            };
        if (status === "pending")
            return {
                fg: "var(--tr-warning)",
                bg: "var(--tr-warning-bg)",
                line: "var(--tr-warning-line)",
                glow: "var(--tr-warning-glow)",
                label: "Wartend",
            };
        return {
            fg: "var(--tr-red)",
            bg: "var(--tr-red-bg)",
            line: "var(--tr-red-line)",
            glow: "var(--tr-red-glow)",
            label: "Belegt",
        };
    }

    function userInitials(user: string): string {
        return (
            user
                .split(/[.\s_-]/)
                .filter(Boolean)
                .map((p) => p[0]?.toUpperCase() ?? "")
                .slice(0, 2)
                .join("") || "?"
        );
    }

    let utilization = $derived(
        ramps.length > 0
            ? Math.round(
                  ((counts.pending + counts.closed) / ramps.length) * 100,
              )
            : 0,
    );

    let userRole = $state("member_lager");
    let isBuero = $derived(userRole === "admin" || userRole === "member_buero");
    let isAdmin = $derived(userRole === "admin");
    let focusTrap: HTMLElement;
    let rampsA = $derived(
        [...ramps]
            .filter((r) => r.id >= 30 && r.id <= 42)
            .sort((a, b) => b.id - a.id),
    );
    let rampsB = $derived(
        [...ramps]
            .filter((r) => r.id >= 43 && r.id <= 57)
            .sort((a, b) => b.id - a.id),
    );

    function releaseFocus() {
        requestAnimationFrame(() => focusTrap?.focus());
    }

    // ── Inline field editing ──
    type EditField = "kennzeichen" | "notiz" | "reserviert_fuer";
    let editingField = $state<{
        rampId: number;
        field: EditField;
        value: string;
    } | null>(null);

    function startFieldEdit(ramp: Ramp, field: EditField) {
        if (isUpdating || isRampLocked(ramp) || !isConnected) return;
        editingField = { rampId: ramp.id, field, value: ramp[field] ?? "" };
    }

    async function commitFieldEdit(ramp: Ramp) {
        if (!editingField || editingField.rampId !== ramp.id) return;
        const { field, value } = editingField;
        editingField = null;
        const trimmed = value.trim();
        if (trimmed === (ramp[field] ?? "")) {
            releaseFocus();
            return;
        }
        const updatedRamp: Ramp = {
            ...ramp,
            [field]: trimmed || null,
            last_updated_by: currentUser,
            // last_updated_at intentionally NOT updated — it tracks when the status
            // last changed and is the source of truth for the dwell timer
        };
        const index = ramps.findIndex((r) => r.id === ramp.id);
        const snapshot = index !== -1 ? { ...ramps[index] } : null;
        if (index !== -1) ramps[index] = updatedRamp;
        isUpdating = true;
        try {
            ramps = await invoke("update_ramp", { updatedRamp });
            lastVersion = null; // resync version cursor on next poll
        } catch (e) {
            console.error("Failed to save field:", e);
            if (index !== -1 && snapshot) ramps[index] = snapshot;
            showError(
                `Rampe ${ramp.id}: Feld konnte nicht gespeichert werden.`,
            );
        } finally {
            isUpdating = false;
        }
        releaseFocus();
    }

    function cancelFieldEdit() {
        editingField = null;
    }

    // ── Daily event log ──
    interface RampEvent {
        timestamp: string;
        ramp_id: number;
        from_status: string;
        to_status: string;
        user: string;
        kennzeichen: string | null;
        reserviert_fuer: string | null;
        duration_min: number | null;
    }

    let dailyLog = $state<RampEvent[]>([]);
    let showProtocol = $state(false);

    // ── Pending-status dialog (free → pending for Büro/Admin) ──
    let pendingDialog = $state<{
        ramp: Ramp;
        kennzeichen: string;
        reserviert_fuer: string;
    } | null>(null);

    // ── User management ──
    interface UserEntry {
        username: string;
        role: string;
    }
    let showUserMgmt = $state(false);
    let allUsers = $state<UserEntry[]>([]);
    let userMgmtLoading = $state(false);

    async function openUserMgmt() {
        userMgmtLoading = true;
        showUserMgmt = true;
        try {
            allUsers = await invoke<UserEntry[]>("get_all_users");
        } catch (e) {
            console.error("Failed to load users:", e);
        } finally {
            userMgmtLoading = false;
        }
    }

    async function changeUserRole(username: string, newRole: string) {
        try {
            await invoke("set_user_role", { username, role: newRole });
            allUsers = allUsers.map((u) =>
                u.username === username ? { ...u, role: newRole } : u,
            );
            if (username === currentUser) userRole = newRole;
        } catch (e) {
            console.error("Failed to set user role:", e);
            showError("Rolle konnte nicht gesetzt werden.");
        }
    }

    function roleLabel(role: string): string {
        if (role === "admin") return "Admin";
        if (role === "member_buero") return "Büro";
        return "Lager";
    }

    // Average time (minutes) spent in any occupied state today
    let avgDwellToday = $derived(
        (() => {
            const relevant = dailyLog.filter(
                (e) => e.duration_min !== null && e.from_status !== "free",
            );
            if (!relevant.length) return null;
            return Math.round(
                relevant.reduce((s, e) => s + (e.duration_min ?? 0), 0) /
                    relevant.length,
            );
        })(),
    );

    async function fetchDailyLog() {
        try {
            dailyLog = await invoke<RampEvent[]>("get_daily_log");
        } catch (e) {
            console.error("Failed to fetch daily log:", e);
        }
    }

    async function openProtocol() {
        statPeriod = "day";
        await fetchDailyLog();
        periodEvents = dailyLog;
        showProtocol = true;
    }

    function fmtStatusDE(s: string) {
        if (s === "free") return "Frei";
        if (s === "pending") return "Wartend";
        return "Belegt";
    }

    function fmtDuration(min: number | null) {
        if (min === null) return "—";
        if (min < 60) return `${min} Min`;
        const h = Math.floor(min / 60);
        const m = min % 60;
        return `${h}h ${String(m).padStart(2, "0")}m`;
    }

    // ── Statistics ──
    type StatPeriod = "day" | "week" | "month" | "year";
    let statPeriod = $state<StatPeriod>("day");
    let periodEvents = $state<RampEvent[]>([]);
    let periodLoading = $state(false);

    function getPeriodDates(period: StatPeriod): { from: string; to: string } {
        const today = new Date();
        const to = today.toISOString().slice(0, 10);
        if (period === "week") {
            const from = new Date(today.getTime() - 6 * 86_400_000);
            return { from: from.toISOString().slice(0, 10), to };
        }
        if (period === "month") {
            const from = new Date(today.getFullYear(), today.getMonth(), 1);
            return { from: from.toISOString().slice(0, 10), to };
        }
        if (period === "year") {
            const from = new Date(today.getFullYear(), 0, 1);
            return { from: from.toISOString().slice(0, 10), to };
        }
        return { from: to, to }; // day
    }

    /** Elapsed minutes from start of selected period until now — denominator for utilization %. */
    function getPeriodMinutes(period: StatPeriod): number {
        const now = new Date();
        let start: Date;
        if (period === "week") {
            start = new Date(
                now.getFullYear(),
                now.getMonth(),
                now.getDate() - 6,
            );
        } else if (period === "month") {
            start = new Date(now.getFullYear(), now.getMonth(), 1);
        } else if (period === "year") {
            start = new Date(now.getFullYear(), 0, 1);
        } else {
            start = new Date(
                now.getFullYear(),
                now.getMonth(),
                now.getDate(),
            );
        }
        return Math.max(1, (now.getTime() - start.getTime()) / 60_000);
    }

    async function fetchPeriodEvents(period: StatPeriod) {
        if (period === "day") {
            periodEvents = dailyLog;
            return;
        }
        periodLoading = true;
        try {
            const { from, to } = getPeriodDates(period);
            periodEvents = await invoke<RampEvent[]>("get_events_for_period", {
                fromDate: from,
                toDate: to,
            });
        } catch (e) {
            console.error("Failed to fetch period events:", e);
        } finally {
            periodLoading = false;
        }
    }

    async function selectPeriod(p: StatPeriod) {
        statPeriod = p;
        await fetchPeriodEvents(p);
    }

    function computeStats(events: RampEvent[], period: StatPeriod) {
        const completions = events.filter((e) => e.to_status === "free").length;
        const dwellEvts = events.filter(
            (e) => e.duration_min !== null && e.from_status !== "free",
        );
        const avgDwell = dwellEvts.length
            ? Math.round(
                  dwellEvts.reduce((s, e) => s + (e.duration_min ?? 0), 0) /
                      dwellEvts.length,
              )
            : null;

        // Average waiting time: how long LKWs sat in 'pending' before status changed
        const waitEvts = events.filter(
            (e) => e.duration_min !== null && e.from_status === "pending",
        );
        const avgWait = waitEvts.length
            ? Math.round(
                  waitEvts.reduce((s, e) => s + (e.duration_min ?? 0), 0) /
                      waitEvts.length,
              )
            : null;

        // Buckets
        const byDay = new Map<string, number>();
        const completionsByDay = new Map<string, number>();
        const byRamp = new Map<number, number>();
        const byUser = new Map<string, number>();
        const byHour = new Map<number, number>();
        // Closed-minutes per ramp (numerator for utilization)
        const closedMinByRamp = new Map<number, number>();

        for (const e of events) {
            const d = e.timestamp.slice(0, 10);
            byDay.set(d, (byDay.get(d) ?? 0) + 1);
            if (e.to_status === "free") {
                completionsByDay.set(d, (completionsByDay.get(d) ?? 0) + 1);
            }
            byRamp.set(e.ramp_id, (byRamp.get(e.ramp_id) ?? 0) + 1);
            byUser.set(e.user, (byUser.get(e.user) ?? 0) + 1);
            const hour = new Date(e.timestamp).getHours();
            if (!isNaN(hour)) {
                byHour.set(hour, (byHour.get(hour) ?? 0) + 1);
            }
            if (e.from_status === "closed" && e.duration_min !== null) {
                closedMinByRamp.set(
                    e.ramp_id,
                    (closedMinByRamp.get(e.ramp_id) ?? 0) + e.duration_min,
                );
            }
        }

        const topRampEntry =
            [...byRamp.entries()].sort((a, b) => b[1] - a[1])[0] ?? null;

        const periodMinutes = getPeriodMinutes(period);
        // Build a utilization map for every ramp seen in events (cap 100% — long-running closed
        // intervals can leak across period boundaries when the closing event lands in-period).
        const utilByRamp = new Map<number, number>();
        for (const [id, mins] of closedMinByRamp.entries()) {
            utilByRamp.set(
                id,
                Math.min(100, Math.round((mins / periodMinutes) * 100)),
            );
        }

        // Outliers: top-3 longest waits (pending dwell) and longest occupancies (closed dwell)
        const longestWaits = events
            .filter((e) => e.from_status === "pending" && e.duration_min !== null)
            .sort((a, b) => (b.duration_min ?? 0) - (a.duration_min ?? 0))
            .slice(0, 3);
        const longestOccupancies = events
            .filter((e) => e.from_status === "closed" && e.duration_min !== null)
            .sort((a, b) => (b.duration_min ?? 0) - (a.duration_min ?? 0))
            .slice(0, 3);

        return {
            totalEvents: events.length,
            completions,
            avgDwell,
            avgWait,
            byDay,
            completionsByDay,
            byRamp,
            byUser,
            byHour,
            utilByRamp,
            closedMinByRamp,
            periodMinutes,
            topRampEntry,
            longestWaits,
            longestOccupancies,
        };
    }

    let periodStats = $derived(computeStats(periodEvents, statPeriod));

    function exportEventsCsv() {
        const cols = [
            "Datum",
            "Zeit",
            "Rampe",
            "Vorher",
            "Nachher",
            "Dauer (min)",
            "Mitarbeiter",
            "Kennzeichen",
            "Reserviert für",
        ];
        // Excel-DE wants ; separator and CRLF; wrap fields containing ; " or newline
        const escape = (v: string | number | null | undefined) => {
            const s = v === null || v === undefined ? "" : String(v);
            return /[";\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
        };
        const rows = periodEvents.map((e) => {
            const d = new Date(e.timestamp);
            const date = e.timestamp.slice(0, 10);
            const time = isNaN(d.getTime())
                ? e.timestamp.slice(11, 19)
                : d.toLocaleTimeString("de-DE", {
                      hour: "2-digit",
                      minute: "2-digit",
                      second: "2-digit",
                  });
            return [
                date,
                time,
                e.ramp_id,
                fmtStatusDE(e.from_status),
                fmtStatusDE(e.to_status),
                e.duration_min ?? "",
                e.user,
                e.kennzeichen ?? "",
                e.reserviert_fuer ?? "",
            ]
                .map(escape)
                .join(";");
        });
        // BOM so Excel auto-detects UTF-8
        const csv = "﻿" + [cols.join(";"), ...rows].join("\r\n");
        const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
        const { from, to } = getPeriodDates(statPeriod);
        const fname = `ramp-protokoll_${from}_bis_${to}.csv`;
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = fname;
        document.body.appendChild(a);
        a.click();
        a.remove();
        setTimeout(() => URL.revokeObjectURL(url), 0);
    }

    function focusInput(node: HTMLInputElement) {
        node.focus();
        node.select();
        return {};
    }
</script>

<!-- Error toast -->
{#if syncError}
    <div
        class="fixed top-4 left-1/2 -translate-x-1/2 z-50 text-white text-sm font-medium px-4 py-2 rounded-lg shadow-xl"
        style="background: var(--tr-red);"
    >
        {syncError}
    </div>
{/if}

<div
    class="h-screen w-screen overflow-hidden flex flex-col"
    style="background: var(--tr-bg); color: var(--tr-text); font-family: 'Inter Variable', sans-serif;"
>
    <!-- ── Header ── -->
    <header
        class="flex items-center gap-4 px-5 shrink-0"
        style="height: 60px; background: var(--tr-always-dark); border-bottom: 1px solid var(--tr-line);"
    >
        <!-- Logo + Title -->
        <div class="flex items-center gap-3">
            <div
                class="w-8 h-8 rounded-md flex items-center justify-center font-bold text-sm"
                style="background: var(--tr-red); color: #fff;"
            >
                T
            </div>
            <div>
                <div
                    class="text-[15px] font-semibold tracking-[-0.2px] leading-none"
                    style="color: #fff;"
                >
                    Troiber<span
                        class="font-normal ml-2"
                        style="color: rgba(255,255,255,0.5);"
                        >/ Ramp Control</span
                    >
                </div>
                <div
                    class="font-mono text-[10.5px] uppercase tracking-[1.4px] mt-[3px]"
                    style="color: rgba(255,255,255,0.45);"
                >
                    Werk Hofkirchen · nTb
                </div>
            </div>
        </div>

        <div class="flex-1"></div>

        <!-- Sync pill -->
        <div
            class="flex items-center gap-2 px-3 py-1.5 rounded-full font-mono text-[11px] tracking-[0.6px]"
            style="background: rgba(255,255,255,0.06); border: 1px solid rgba(255,255,255,0.08); color: rgba(255,255,255,0.7);"
        >
            <span
                class="w-[7px] h-[7px] rounded-full"
                class:opacity-50={!isConnected}
                style="background: var(--tr-green); box-shadow: 0 0 0 3px {isConnected
                    ? 'rgba(104,197,47,0.15)'
                    : 'transparent'}, 0 0 8px {isConnected
                    ? 'rgba(104,197,47,0.6)'
                    : 'transparent'};"
            ></span>
            {isConnected ? "LIVE · 1.5s" : "OFFLINE"}
        </div>

        <!-- Status chips -->
        <div class="flex gap-1.5">
            {#each [{ status: "free", label: "Frei", count: counts.free }, { status: "pending", label: "Wartend", count: counts.pending }, { status: "closed", label: "Belegt", count: counts.closed }] as chip}
                {@const tone = getStatusTone(chip.status)}
                <div
                    class="flex items-baseline gap-2 px-3 py-1.5 rounded-lg font-mono min-w-[82px]"
                    style="background: {tone.bg}; border: 1px solid {tone.line};"
                >
                    <span
                        class="text-[18px] font-semibold leading-none tabular-nums"
                        style="color: {tone.fg};"
                    >
                        {String(chip.count).padStart(2, "0")}
                    </span>
                    <span
                        class="text-[11px] font-semibold uppercase tracking-[0.6px]"
                        style="color: {tone.fg};"
                    >
                        {chip.label}
                    </span>
                </div>
            {/each}
        </div>

        <div
            class="w-px h-6 mx-1"
            style="background: rgba(255,255,255,0.1);"
        ></div>

        <!-- Protokoll button — Büromodus only -->
        {#if isBuero}
            <button
                onclick={openProtocol}
                class="h-[34px] px-4 rounded-[10px] flex items-center gap-2 text-[13px] font-medium cursor-pointer"
                style="border: 1px solid rgba(255,255,255,0.1); background: rgba(255,255,255,0.04); color: rgba(255,255,255,0.9);"
                aria-label="Tagesprotokoll öffnen"
            >
                <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.8"
                    stroke-linecap="round"
                    ><path
                        d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"
                    /><polyline points="14 2 14 8 20 8" /><line
                        x1="16"
                        y1="13"
                        x2="8"
                        y2="13"
                    /><line x1="16" y1="17" x2="8" y2="17" /><polyline
                        points="10 9 9 9 8 9"
                    /></svg
                >
                Protokoll
                {#if dailyLog.length > 0}
                    <span
                        class="font-mono text-[10px]"
                        style="color: rgba(255,255,255,0.45);"
                        >{dailyLog.length}</span
                    >
                {/if}
            </button>
        {/if}

        <!-- Admin: user management button -->
        {#if isAdmin}
            <button
                onclick={openUserMgmt}
                class="h-[34px] px-4 rounded-[10px] flex items-center gap-2 text-[13px] font-medium cursor-pointer"
                style="border: 1px solid rgba(255,255,255,0.1); background: rgba(255,255,255,0.04); color: rgba(255,255,255,0.9);"
                aria-label="Benutzerverwaltung öffnen"
            >
                <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.8"
                    stroke-linecap="round"
                    ><path
                        d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"
                    /><circle cx="9" cy="7" r="4" /><path
                        d="M23 21v-2a4 4 0 0 0-3-3.87"
                    /><path d="M16 3.13a4 4 0 0 1 0 7.75" /></svg
                >
                Benutzer
            </button>
        {/if}

        <!-- Role badge -->
        {#if isAdmin}
            <div
                class="flex items-center gap-1.5 px-3 py-1 rounded-lg font-mono text-[11px] font-semibold uppercase"
                style="background: rgba(218,43,41,0.2); border: 1px solid rgba(218,43,41,0.4); color: #FF6B69; letter-spacing: 1px;"
            >
                ADMIN
            </div>
        {:else if userRole === "member_buero"}
            <div
                class="flex items-center gap-1.5 px-3 py-1 rounded-lg font-mono text-[11px] font-semibold uppercase"
                style="background: rgba(218,43,41,0.15); border: 1px solid rgba(218,43,41,0.3); color: #FF6B69; letter-spacing: 1px;"
            >
                BÜRO
            </div>
        {/if}

        <!-- Theme toggle -->
        <button
            onclick={toggleMode}
            class="w-[34px] h-[34px] rounded-[10px] grid place-items-center cursor-pointer transition-colors"
            style="border: 1px solid rgba(255,255,255,0.1); background: rgba(255,255,255,0.04); color: rgba(255,255,255,0.7);"
            aria-label="Theme wechseln"
        >
            <svg
                width="15"
                height="15"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                class="dark:hidden"
                ><circle cx="12" cy="12" r="4" /><path
                    d="M12 3v1M12 20v1M4.93 4.93l.71.71M18.36 18.36l.71.71M3 12h1M20 12h1M4.93 19.07l.71-.71M18.36 5.64l.71-.71"
                /></svg
            >
            <svg
                width="15"
                height="15"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="hidden dark:block"
                ><path
                    d="M20.5 14.2A8 8 0 1 1 9.8 3.5a6 6 0 0 0 10.7 10.7z"
                /></svg
            >
        </button>

        <!-- Chat toggle -->
        <button
            onclick={chatOpen ? closeChat : openChat}
            class="relative h-[34px] px-4 rounded-[10px] flex items-center gap-2 text-[13px] font-medium cursor-pointer transition-colors"
            style="border: 1px solid {chatOpen
                ? 'var(--tr-red)'
                : 'rgba(255,255,255,0.1)'}; background: {chatOpen
                ? 'rgba(218,43,41,0.15)'
                : 'rgba(255,255,255,0.04)'}; color: {chatOpen
                ? '#FF6B69'
                : 'rgba(255,255,255,0.9)'};"
        >
            <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                ><path
                    d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"
                /></svg
            >
            Team
            {#if unreadCount > 0}
                <span
                    class="min-w-[18px] h-[18px] px-[5px] rounded-full grid place-items-center text-[10px] font-semibold font-mono"
                    style="background: var(--tr-red); color: #fff;"
                    >{unreadCount}</span
                >
            {/if}
        </button>

        <!-- User chip -->
        <div
            class="flex items-center gap-2.5 pl-1 pr-3 rounded-full"
            style="background: rgba(255,255,255,0.04); border: 1px solid rgba(255,255,255,0.08);"
        >
            <div
                class="w-[26px] h-[26px] rounded-full grid place-items-center text-[11.5px] font-semibold"
                style="background: var(--tr-red); color: #fff;"
            >
                {userInitials(currentUser)}
            </div>
            <div class="leading-[1.15]">
                <div
                    class="text-[12.5px] font-medium"
                    style="color: rgba(255,255,255,0.9);"
                >
                    {currentUser}
                </div>
                <div
                    class="font-mono text-[10px] uppercase tracking-[0.8px] mt-[1px]"
                    style="color: rgba(255,255,255,0.45);"
                >
                    {roleLabel(userRole)}
                </div>
            </div>
        </div>
    </header>

    <!-- ── Body ── -->
    <div class="flex-1 overflow-hidden relative">
        <!-- Main scroll area -->
        <main class="h-full overflow-auto" style="padding: 22px 26px;">
            <div
                bind:this={focusTrap}
                tabindex="-1"
                class="sr-only"
                aria-hidden="true"
            ></div>
            <!-- Section header -->
            <div class="flex items-end justify-between mb-5 gap-6">
                <div>
                    <div class="flex items-center gap-2.5 mb-2">
                        <span
                            class="font-mono text-[11px] uppercase tracking-[1.6px] font-medium"
                            style="color: var(--tr-text-faint);"
                            >Werk Hofkirchen · nTb</span
                        >
                        <span
                            class="w-4 h-px"
                            style="background: var(--tr-line);"
                        ></span>
                        <span
                            class="font-mono text-[11px] uppercase tracking-[1.6px] font-medium"
                            style="color: var(--tr-text-faint);"
                            >{ramps.length} Rampen</span
                        >
                    </div>
                    <h1 class="text-[24px] font-semibold tracking-[-0.4px] m-0">
                        Rampenbelegung
                    </h1>
                </div>
                <div
                    class="flex items-center gap-2 text-[11.5px]"
                    style="color: var(--tr-text-faint);"
                >
                    <kbd
                        class="px-[7px] py-[2px] rounded-[5px] font-mono text-[10.5px]"
                        style="border: 1px solid var(--tr-line); color: var(--tr-text-dim); background: var(--tr-surface);"
                        >Klick</kbd
                    >
                    Status · KFZ anklicken zum Bearbeiten
                </div>
            </div>

            <!-- Ramp tile snippet -->
            {#snippet rampTile(ramp: Ramp)}
                {@const locked = !!ramp.locked_until && ramp.locked_until > now}
                {@const tone = getStatusTone(ramp.status)}
                {@const isFree = ramp.status === "free"}
                {@const dwellSec =
                    !isFree && ramp.last_updated_at
                        ? Math.max(
                              0,
                              Math.floor(
                                  (now - parseTsMs(ramp.last_updated_at)) /
                                      1000,
                              ),
                          )
                        : 0}
                {@const dwellH = Math.floor(dwellSec / 3600)}
                {@const dwellM = Math.floor((dwellSec % 3600) / 60)}
                {@const dwellS = dwellSec % 60}
                {@const dwell =
                    dwellH > 0
                        ? `${String(dwellH).padStart(2, "0")}:${String(dwellM).padStart(2, "0")}:${String(dwellS).padStart(2, "0")}`
                        : `${String(dwellM).padStart(2, "0")}:${String(dwellS).padStart(2, "0")}`}
                {@const dt = ramp.last_updated_at
                    ? new Date(ramp.last_updated_at)
                    : null}
                {@const timeStr = dt
                    ? dt.toLocaleTimeString("de-DE", {
                          hour: "2-digit",
                          minute: "2-digit",
                      })
                    : null}
                {@const canEdit = isConnected && !isUpdating && !locked}
                {@const isEditKfz =
                    editingField?.rampId === ramp.id &&
                    editingField?.field === "kennzeichen"}
                {@const isEditRes =
                    editingField?.rampId === ramp.id &&
                    editingField?.field === "reserviert_fuer"}

                <!-- div+role avoids invalid nested-interactive HTML -->
                <div
                    class="ramp-tile relative rounded-[12px] flex flex-col gap-[7px]"
                    role="button"
                    tabindex="0"
                    aria-label="Rampe {ramp.id}, {getStatusLabel(
                        ramp.status,
                    )}. Enter um Status zu ändern."
                    onclick={(e) => {
                        // Only block clicks that land directly on an active input
                        // Field buttons handle their own stopPropagation
                        if ((e.target as HTMLElement).tagName === "INPUT")
                            return;
                        if (canEdit) cycleStatus(ramp);
                    }}
                    onkeydown={(e) => {
                        if ((e.target as HTMLElement).tagName === "INPUT")
                            return;
                        if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault();
                            if (canEdit) cycleStatus(ramp);
                        }
                    }}
                    style="
                        padding: 10px 10px 8px;
                        cursor: {canEdit ? 'pointer' : 'default'};
                        background: {isFree
                        ? 'var(--tr-surface)'
                        : 'linear-gradient(180deg, ' +
                          tone.bg +
                          ' 0%, var(--tr-surface) 72%)'};
                        border: 1px solid {isFree
                        ? 'var(--tr-line)'
                        : tone.line};
                        box-shadow: {isFree
                        ? 'none'
                        : '0 0 24px -10px ' + tone.glow};
                    "
                >
                    <!-- Lock overlay -->
                    {#if locked}
                        <div
                            class="absolute inset-0 z-30 rounded-[12px] flex items-center justify-center backdrop-blur-[1px]"
                            style="background: rgba(0,0,0,0.28);"
                        >
                            <LoaderCircleIcon
                                class="w-5 h-5 animate-spin"
                                style="color:#fff"
                            />
                        </div>
                    {/if}

                    <!-- Top accent stripe -->
                    <div
                        class="absolute top-0 left-0 right-0 h-[3px] rounded-t-[12px]"
                        style="background: {tone.fg}; opacity: {isFree
                            ? 0.35
                            : 1};"
                    ></div>

                    <!-- Row 1: number + status badge -->
                    <div class="flex items-start justify-between gap-1.5">
                        <div
                            class="font-semibold leading-none tabular-nums"
                            style="font-size: 26px; letter-spacing: -1px; color: var(--tr-text);"
                        >
                            {ramp.id}
                        </div>
                        <span
                            class="inline-flex items-center gap-[4px] px-[6px] py-[4px] rounded-[5px] font-mono text-[9px] font-semibold uppercase tracking-[0.5px]"
                            style="background: {tone.bg}; border: 1px solid {tone.line}; color: {tone.fg};"
                        >
                            <span
                                class="w-[5px] h-[5px] rounded-full flex-shrink-0"
                                style="background: {tone.fg}; box-shadow: {!isFree
                                    ? '0 0 5px ' + tone.glow
                                    : 'none'}; animation: {ramp.status ===
                                'pending'
                                    ? 'mc-pulse 1.6s ease-in-out infinite'
                                    : 'none'};"
                            ></span>
                            {tone.label}
                        </span>
                    </div>

                    <!-- Row 2: Kennzeichen -->
                    <div
                        data-field="kennzeichen"
                        class="rounded-[4px] overflow-hidden"
                        style="border: 1px solid {isEditKfz
                            ? 'var(--tr-warning)'
                            : 'var(--tr-line)'}; background: var(--tr-surface2);"
                    >
                        {#if isEditKfz}
                            <div
                                class="flex items-center gap-1 px-[7px] py-[4px]"
                            >
                                <span
                                    class="font-mono text-[8px] font-semibold uppercase flex-shrink-0"
                                    style="letter-spacing:1px; color:var(--tr-text-faint);"
                                    >KFZ</span
                                >
                                <input
                                    type="text"
                                    class="flex-1 min-w-0 font-mono text-[11px] font-medium bg-transparent outline-none"
                                    style="color:var(--tr-text); letter-spacing:0.5px;"
                                    value={editingField!.value}
                                    maxlength={15}
                                    placeholder="z.B. M-TR 2418"
                                    oninput={(e) => {
                                        if (editingField)
                                            editingField.value =
                                                e.currentTarget.value;
                                    }}
                                    onblur={() => commitFieldEdit(ramp)}
                                    onkeydown={(e) => {
                                        if (e.key === "Enter") {
                                            e.preventDefault();
                                            commitFieldEdit(ramp);
                                        }
                                        if (e.key === "Escape") {
                                            e.preventDefault();
                                            cancelFieldEdit();
                                        }
                                    }}
                                    onclick={(e) => e.stopPropagation()}
                                    use:focusInput
                                />
                            </div>
                        {:else}
                            <button
                                class="w-full flex items-center gap-1 px-[7px] py-[4px] text-left"
                                style="background:transparent; border:none; cursor:{canEdit
                                    ? 'text'
                                    : 'default'};"
                                disabled={!canEdit}
                                onclick={(e) => {
                                    e.stopPropagation();
                                    startFieldEdit(ramp, "kennzeichen");
                                }}
                                aria-label="Kennzeichen bearbeiten"
                                tabindex="-1"
                            >
                                <span
                                    class="font-mono text-[8px] font-semibold uppercase flex-shrink-0"
                                    style="letter-spacing:1px; color:var(--tr-text-faint);"
                                    >LKW</span
                                >
                                <span
                                    class="font-mono text-[11px] font-medium truncate"
                                    style="color:{ramp.kennzeichen
                                        ? 'var(--tr-text)'
                                        : 'var(--tr-text-faint)'}; letter-spacing:0.5px;"
                                >
                                    {ramp.kennzeichen || "— — —"}
                                </span>
                            </button>
                        {/if}
                    </div>

                    <!-- Row 3: Reserviert-für (only in Büro mode or when set) -->
                    {#if isBuero || ramp.reserviert_fuer}
                        <div
                            data-field="reserviert_fuer"
                            class="rounded-[4px] overflow-hidden"
                            style="border: 1px solid {isEditRes
                                ? 'var(--tr-warning)'
                                : 'var(--tr-line)'}; background: var(--tr-surface2);"
                        >
                            {#if isEditRes}
                                <div
                                    class="flex items-center gap-1 px-[7px] py-[4px]"
                                >
                                    <span
                                        class="font-mono text-[8px] font-semibold uppercase flex-shrink-0"
                                        style="letter-spacing:1px; color:var(--tr-text-faint);"
                                        >Res.</span
                                    >
                                    <input
                                        type="text"
                                        class="flex-1 min-w-0 text-[11px] bg-transparent outline-none"
                                        style="color:var(--tr-text);"
                                        value={editingField!.value}
                                        maxlength={40}
                                        placeholder="Reserviert für…"
                                        oninput={(e) => {
                                            if (editingField)
                                                editingField.value =
                                                    e.currentTarget.value;
                                        }}
                                        onblur={() => commitFieldEdit(ramp)}
                                        onkeydown={(e) => {
                                            if (e.key === "Enter") {
                                                e.preventDefault();
                                                commitFieldEdit(ramp);
                                            }
                                            if (e.key === "Escape") {
                                                e.preventDefault();
                                                cancelFieldEdit();
                                            }
                                        }}
                                        onclick={(e) => e.stopPropagation()}
                                        use:focusInput
                                    />
                                </div>
                            {:else if isBuero}
                                <button
                                    class="w-full flex items-center gap-1 px-[7px] py-[4px] text-left"
                                    style="background:transparent; border:none; cursor:{canEdit
                                        ? 'text'
                                        : 'default'};"
                                    disabled={!canEdit}
                                    onclick={(e) => {
                                        e.stopPropagation();
                                        startFieldEdit(ramp, "reserviert_fuer");
                                    }}
                                    aria-label="Reserviert-für bearbeiten"
                                    tabindex="-1"
                                >
                                    <span
                                        class="font-mono text-[8px] font-semibold uppercase flex-shrink-0"
                                        style="letter-spacing:1px; color:var(--tr-text-faint);"
                                        >Res.</span
                                    >
                                    <span
                                        class="text-[11px] truncate"
                                        style="color:{ramp.reserviert_fuer
                                            ? 'var(--tr-text-dim)'
                                            : 'var(--tr-text-faint)'};"
                                    >
                                        {ramp.reserviert_fuer || "—"}
                                    </span>
                                </button>
                            {:else}
                                <!-- Normal mode, read-only display when value is set -->
                                <div
                                    class="flex items-center gap-1 px-[7px] py-[4px]"
                                >
                                    <span
                                        class="font-mono text-[8px] font-semibold uppercase flex-shrink-0"
                                        style="letter-spacing:1px; color:var(--tr-text-faint);"
                                        >Res.</span
                                    >
                                    <span
                                        class="text-[11px] truncate"
                                        style="color:var(--tr-text-dim);"
                                    >
                                        {ramp.reserviert_fuer}
                                    </span>
                                </div>
                            {/if}
                        </div>
                    {/if}

                    <!-- Row 4: dwell / time -->
                    <div class="mt-auto flex justify-between items-end pt-0.5">
                        {#if !isFree}
                            <div>
                                <div
                                    class="font-mono text-[8px] uppercase mb-0.5"
                                    style="letter-spacing:1px; color:var(--tr-text-faint);"
                                >
                                    Verweildauer
                                </div>
                                <div
                                    class="font-mono text-[14px] font-semibold leading-none tabular-nums"
                                    style="color:{tone.fg};"
                                >
                                    {dwell}
                                </div>
                            </div>
                            <div class="text-right">
                                <div
                                    class="font-mono text-[8px] uppercase mb-0.5"
                                    style="letter-spacing:1px; color:var(--tr-text-faint);"
                                >
                                    Seit
                                </div>
                                <div
                                    class="text-[11px] font-medium"
                                    style="color:var(--tr-text-dim);"
                                >
                                    {timeStr ?? "—"}
                                </div>
                            </div>
                        {:else}
                            <div
                                class="font-mono text-[9px]"
                                style="color:var(--tr-text-faint); letter-spacing:0.4px;"
                            >
                                {timeStr
                                    ? "Frei seit " + timeStr
                                    : "Noch nie belegt"}
                            </div>
                        {/if}
                    </div>

                    <!-- Row 5: worker -->
                    {#if ramp.last_updated_by}
                        <div
                            class="flex items-baseline gap-1 pt-1 border-t"
                            style="border-color: var(--tr-line);"
                        >
                            <span
                                class="font-mono text-[8px] uppercase flex-shrink-0"
                                style="letter-spacing:1px; color:var(--tr-text-faint);"
                                >Von</span
                            >
                            <span
                                class="text-[11px] font-medium truncate"
                                style="color:var(--tr-text-dim);"
                                title={ramp.last_updated_by}
                            >
                                {ramp.last_updated_by}
                            </span>
                        </div>
                    {/if}
                </div>
            {/snippet}

            <!-- Ramp grid -->
            {#if ramps.length === 0}
                <div
                    class="flex flex-col items-center justify-center py-24 gap-3"
                    style="color: var(--tr-text-faint);"
                >
                    <div
                        class="w-8 h-8 border-2 border-t-[color:var(--tr-green)] rounded-full animate-spin"
                        style="border-color: var(--tr-line); border-top-color: var(--tr-green);"
                    ></div>
                    <p class="text-sm font-medium">Verbinde…</p>
                </div>
            {:else}
                <!-- Section A -->
                <div class="mb-5">
                    <div
                        class="font-mono text-[10px] uppercase tracking-[1.6px] font-medium mb-2"
                        style="color: var(--tr-text-faint);"
                    >
                        01 / Sektor A · Tor 30–42 — {rampsA.length} Rampen
                    </div>
                    <div
                        style="display: grid; grid-template-columns: repeat(auto-fill, minmax(148px, 1fr)); gap: 8px;"
                    >
                        {#each rampsA as ramp (ramp.id)}
                            {@render rampTile(ramp)}
                        {/each}
                    </div>
                </div>

                <!-- Section B -->
                <div>
                    <div
                        class="font-mono text-[10px] uppercase tracking-[1.6px] font-medium mb-2"
                        style="color: var(--tr-text-faint);"
                    >
                        02 / Sektor B · Tor 43–57 — {rampsB.length} Rampen
                    </div>
                    <div
                        style="display: grid; grid-template-columns: repeat(auto-fill, minmax(148px, 1fr)); gap: 8px;"
                    >
                        {#each rampsB as ramp (ramp.id)}
                            {@render rampTile(ramp)}
                        {/each}
                    </div>
                </div>
            {/if}

            <!-- Stats strip -->
            {#if ramps.length > 0}
                <div
                    class="mt-5 rounded-[12px] grid gap-7"
                    style="padding: 16px 20px; background: var(--tr-surface); border: 1px solid var(--tr-line); grid-template-columns: 1fr 1fr 1fr 1fr;"
                >
                    <div class="flex flex-col gap-1.5">
                        <div
                            class="font-mono text-[10px] uppercase tracking-[1.2px] font-medium"
                            style="color: var(--tr-text-faint);"
                        >
                            Ø Verweildauer heute
                        </div>
                        <div class="flex items-baseline gap-1.5">
                            <span
                                class="font-mono text-[22px] font-semibold leading-none tabular-nums"
                                style="color: {avgDwellToday !== null
                                    ? 'var(--tr-warning)'
                                    : 'var(--tr-text)'};"
                            >
                                {avgDwellToday !== null
                                    ? String(
                                          Math.floor(avgDwellToday / 60),
                                      ).padStart(2, "0") +
                                      ":" +
                                      String(avgDwellToday % 60).padStart(
                                          2,
                                          "0",
                                      )
                                    : "—"}
                            </span>
                            <span
                                class="text-[10px] uppercase tracking-[0.6px]"
                                style="color: var(--tr-text-faint);"
                                >{avgDwellToday !== null
                                    ? "HH:MM"
                                    : "keine Daten"}</span
                            >
                        </div>
                    </div>
                    <div class="flex flex-col gap-1.5">
                        <div
                            class="font-mono text-[10px] uppercase tracking-[1.2px] font-medium"
                            style="color: var(--tr-text-faint);"
                        >
                            Wartend
                        </div>
                        <div class="flex items-baseline gap-1.5">
                            <span
                                class="font-mono text-[22px] font-semibold leading-none tabular-nums"
                                style="color: var(--tr-warning);"
                                >{String(counts.pending).padStart(2, "0")}</span
                            >
                            <span
                                class="text-[10px] uppercase tracking-[0.6px]"
                                style="color: var(--tr-text-faint);">LKW</span
                            >
                        </div>
                    </div>
                    <div class="flex flex-col gap-1.5">
                        <div
                            class="font-mono text-[10px] uppercase tracking-[1.2px] font-medium"
                            style="color: var(--tr-text-faint);"
                        >
                            Belegt
                        </div>
                        <div class="flex items-baseline gap-1.5">
                            <span
                                class="font-mono text-[22px] font-semibold leading-none tabular-nums"
                                style="color: var(--tr-red);"
                                >{String(counts.closed).padStart(2, "0")}</span
                            >
                            <span
                                class="text-[10px] uppercase tracking-[0.6px]"
                                style="color: var(--tr-text-faint);">LKW</span
                            >
                        </div>
                    </div>
                    <div class="flex flex-col gap-1.5">
                        <div
                            class="font-mono text-[10px] uppercase tracking-[1.2px] font-medium"
                            style="color: var(--tr-text-faint);"
                        >
                            Auslastung
                        </div>
                        <div class="flex items-baseline gap-1.5">
                            <span
                                class="font-mono text-[22px] font-semibold leading-none tabular-nums"
                                style="color: var(--tr-text);"
                                >{utilization}%</span
                            >
                            <span
                                class="text-[10px] uppercase tracking-[0.6px]"
                                style="color: var(--tr-text-faint);"
                                >{counts.pending + counts.closed} / {ramps.length}</span
                            >
                        </div>
                    </div>
                </div>
            {/if}
        </main>

        <!-- ── Chat panel ── -->
        <aside
            class="absolute top-0 right-0 bottom-0 overflow-hidden flex flex-col chat-panel"
            style="width: {chatOpen
                ? '360px'
                : '0px'}; background: var(--tr-surface); border-left: 1px solid {chatOpen
                ? 'var(--tr-line)'
                : 'transparent'};"
        >
            <!-- Chat header -->
            <div
                class="flex items-start gap-2.5 px-5 py-[18px] shrink-0"
                style="border-bottom: 1px solid var(--tr-line);"
            >
                <div class="flex-1 min-w-0">
                    <div
                        class="text-[14px] font-semibold tracking-[-0.1px]"
                        style="color: var(--tr-text);"
                    >
                        Team Chat
                    </div>
                    <div
                        class="font-mono text-[10.5px] uppercase tracking-[0.6px] mt-[3px]"
                        style="color: var(--tr-text-faint);"
                    >
                        {messages.length} Nachrichten
                    </div>
                </div>
                <button
                    onclick={closeChat}
                    aria-label="Chat schließen"
                    class="w-7 h-7 rounded-[7px] grid place-items-center cursor-pointer transition-colors"
                    style="border: 1px solid var(--tr-line); background: transparent; color: var(--tr-text-dim);"
                >
                    <svg
                        width="12"
                        height="12"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"><path d="M18 6L6 18M6 6l12 12" /></svg
                    >
                </button>
            </div>

            <!-- Messages -->
            <div
                bind:this={chatScrollEl}
                class="flex-1 overflow-y-auto chat-scroll flex flex-col gap-4 min-h-0"
                style="padding: 16px 18px;"
            >
                {#if messages.length === 0}
                    <p
                        class="text-xs text-center pt-12"
                        style="color: var(--tr-text-faint);"
                    >
                        Noch keine Nachrichten
                    </p>
                {/if}

                {#each messages as msg, i (msg.id)}
                    {@const isOwn = msg.user === currentUser}
                    {@const prevMsg = i > 0 ? messages[i - 1] : null}
                    {@const isFirst = !prevMsg || prevMsg.user !== msg.user}
                    {@const isDiffDay =
                        !prevMsg ||
                        formatDate(msg.timestamp) !==
                            formatDate(prevMsg.timestamp)}

                    {#if isDiffDay}
                        <div class="flex items-center gap-2 my-1">
                            <div
                                class="flex-1 h-px"
                                style="background: var(--tr-line);"
                            ></div>
                            <span
                                class="font-mono text-[10px] uppercase tracking-[1.2px]"
                                style="color: var(--tr-text-faint);"
                                >{formatDate(msg.timestamp)}</span
                            >
                            <div
                                class="flex-1 h-px"
                                style="background: var(--tr-line);"
                            ></div>
                        </div>
                    {/if}

                    <div
                        class="flex gap-2.5 {isOwn
                            ? 'flex-row-reverse'
                            : 'flex-row'} {isFirst ? 'mt-1' : 'mt-[-6px]'}"
                    >
                        {#if !isOwn}
                            {#if isFirst}
                                <div
                                    class="w-7 h-7 rounded-full grid place-items-center text-[11.5px] font-semibold shrink-0 mb-[2px]"
                                    style="background: var(--tr-surface2); border: 1px solid var(--tr-line); color: var(--tr-text-dim);"
                                >
                                    {msg.user.charAt(0).toUpperCase()}
                                </div>
                            {:else}
                                <div class="w-7 shrink-0"></div>
                            {/if}
                        {/if}

                        <div
                            class="flex flex-col {isOwn
                                ? 'items-end'
                                : 'items-start'} max-w-[78%]"
                        >
                            {#if isFirst && !isOwn}
                                <div
                                    class="font-mono text-[11px] mb-[5px]"
                                    style="color: var(--tr-text-dim);"
                                >
                                    {msg.user}
                                    <span style="color: var(--tr-text-faint);"
                                        >· {formatTime(msg.timestamp)}</span
                                    >
                                </div>
                            {/if}
                            <div
                                class="text-[13px] leading-[1.45] px-[13px] py-2 break-words"
                                style="
                                     background: {isOwn
                                    ? 'var(--tr-red)'
                                    : 'var(--tr-surface2)'};
                                     color: {isOwn ? '#fff' : 'var(--tr-text)'};
                                     border-radius: {isOwn
                                    ? '12px 12px 3px 12px'
                                    : '3px 12px 12px 12px'};
                                     border: {isOwn
                                    ? 'none'
                                    : '1px solid var(--tr-line)'};
                                 "
                            >
                                {msg.text}
                            </div>
                            {#if isOwn}
                                <div
                                    class="font-mono text-[10.5px] mt-[5px]"
                                    style="color: var(--tr-text-faint);"
                                >
                                    {formatTime(msg.timestamp)}
                                </div>
                            {/if}
                        </div>
                    </div>
                {/each}
            </div>

            <!-- Input -->
            <form
                onsubmit={(e) => {
                    e.preventDefault();
                    sendMessage();
                }}
                class="flex gap-2 shrink-0 p-[12px_14px]"
                style="border-top: 1px solid var(--tr-line);"
            >
                <input
                    type="text"
                    bind:value={chatInput}
                    placeholder="Nachricht…"
                    disabled={isSendingMessage}
                    maxlength={300}
                    onkeydown={(e) => {
                        if (e.key === "Escape") e.currentTarget.blur();
                    }}
                    class="flex-1 h-9 px-3 rounded-[8px] text-[13px] outline-none transition-colors disabled:opacity-50"
                    style="border: 1px solid var(--tr-line); background: var(--tr-bg); color: var(--tr-text); font-family: 'Inter Variable', sans-serif;"
                />
                <button
                    type="submit"
                    disabled={isSendingMessage || !chatInput.trim()}
                    aria-label="Senden"
                    class="w-9 h-9 rounded-[8px] grid place-items-center cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed transition-opacity"
                    style="border: none; background: var(--tr-red); color: #fff;"
                >
                    <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        ><path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z" /></svg
                    >
                </button>
            </form>
        </aside>
    </div>
</div>

<!-- ── Protokoll & Statistik Modal ── -->
{#if showProtocol}
    {@const stats = periodStats}
    {@const dayEntries = [...periodEvents].reverse()}
    {@const dayMap = [...stats.completionsByDay.entries()].sort((a, b) =>
        a[0].localeCompare(b[0]),
    )}
    {@const maxDayCount = dayMap.length
        ? Math.max(...dayMap.map(([, n]) => n))
        : 1}
    {@const topRamps = [...stats.byRamp.entries()]
        .map(([id, count]) =>
            [id, count, stats.utilByRamp.get(id) ?? 0] as [number, number, number],
        )
        .sort((a, b) => b[1] - a[1])
        .slice(0, 8)}
    {@const userRows = [...stats.byUser.entries()].sort(
        (a, b) => b[1] - a[1],
    )}
    {@const hourRows = Array.from({ length: 24 }, (_, h) =>
        [h, stats.byHour.get(h) ?? 0] as [number, number],
    )}
    {@const maxHourCount = Math.max(1, ...hourRows.map(([, n]) => n))}
    <div
        class="fixed inset-0 z-50 flex items-center justify-center"
        style="background: rgba(0,0,0,0.6); backdrop-filter: blur(4px);"
        onclick={(e) => {
            if (e.target === e.currentTarget) showProtocol = false;
        }}
        onkeydown={(e) => {
            if (e.key === "Escape") showProtocol = false;
        }}
        role="dialog"
        tabindex="-1"
        aria-modal="true"
        aria-label="Protokoll & Statistik"
    >
        <div
            class="flex flex-col rounded-2xl overflow-hidden shadow-2xl"
            style="width: 1140px; max-width: 96vw; max-height: 88vh; background: var(--tr-surface); border: 1px solid var(--tr-line);"
        >
            <!-- Header -->
            <div
                class="flex items-center gap-4 px-6 py-4 shrink-0"
                style="border-bottom: 1px solid var(--tr-line); background: var(--tr-always-dark);"
            >
                <div class="flex-1">
                    <div class="text-[15px] font-semibold" style="color:#fff;">
                        Protokoll & Statistik
                    </div>
                    <div
                        class="font-mono text-[10.5px] uppercase mt-[3px]"
                        style="letter-spacing:1.4px; color:rgba(255,255,255,0.45);"
                    >
                        {new Date().toLocaleDateString("de-DE", {
                            weekday: "long",
                            day: "2-digit",
                            month: "2-digit",
                            year: "numeric",
                        })}
                        &nbsp;·&nbsp; {stats.totalEvents} Ereignisse
                    </div>
                </div>
                <!-- Period tabs -->
                <div
                    class="flex gap-1"
                    style="background:rgba(255,255,255,0.06); padding:3px; border-radius:8px;"
                >
                    {#each [["day", "Tag"], ["week", "Woche"], ["month", "Monat"], ["year", "Jahr"]] as const as [p, label]}
                        <button
                            onclick={() => selectPeriod(p)}
                            class="px-3 py-1 rounded-[6px] font-mono text-[11px] font-semibold uppercase cursor-pointer transition-colors"
                            style="letter-spacing:0.8px; background:{statPeriod ===
                            p
                                ? 'rgba(255,255,255,0.12)'
                                : 'transparent'}; color:{statPeriod === p
                                ? '#fff'
                                : 'rgba(255,255,255,0.5)'}; border:none;"
                        >
                            {label}
                        </button>
                    {/each}
                </div>
                <button
                    onclick={exportEventsCsv}
                    disabled={periodEvents.length === 0}
                    aria-label="Als CSV exportieren"
                    class="h-8 px-3 rounded-lg flex items-center gap-2 text-[12px] font-medium cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
                    style="border:1px solid rgba(255,255,255,0.15); background:rgba(255,255,255,0.06); color:rgba(255,255,255,0.8);"
                >
                    <svg
                        width="13"
                        height="13"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.8"
                        ><path
                            d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"
                        /><polyline points="7 10 12 15 17 10" /><line
                            x1="12"
                            y1="15"
                            x2="12"
                            y2="3"
                        /></svg
                    >
                    CSV
                </button>
                <button
                    onclick={() => window.print()}
                    aria-label="Drucken"
                    class="h-8 px-3 rounded-lg flex items-center gap-2 text-[12px] font-medium cursor-pointer"
                    style="border:1px solid rgba(255,255,255,0.15); background:rgba(255,255,255,0.06); color:rgba(255,255,255,0.8);"
                >
                    <svg
                        width="13"
                        height="13"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.8"
                        ><polyline points="6 9 6 2 18 2 18 9" /><path
                            d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"
                        /><rect x="6" y="14" width="12" height="8" /></svg
                    >
                    Drucken
                </button>
                <button
                    onclick={() => (showProtocol = false)}
                    aria-label="Schließen"
                    class="w-8 h-8 rounded-lg grid place-items-center cursor-pointer"
                    style="border:1px solid rgba(255,255,255,0.1); background:transparent; color:rgba(255,255,255,0.6);"
                >
                    <svg
                        width="13"
                        height="13"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"><path d="M18 6L6 18M6 6l12 12" /></svg
                    >
                </button>
            </div>

            <!-- Summary strip -->
            <div
                class="flex flex-wrap gap-x-6 gap-y-3 px-6 py-3 shrink-0"
                style="border-bottom:1px solid var(--tr-line); background:var(--tr-surface2);"
            >
                {#each [{ label: "Statuswechsel", value: String(stats.totalEvents), color: "var(--tr-text)" }, { label: "Abfertigungen", value: String(stats.completions), color: "var(--tr-green)" }, { label: "Ø Wartezeit", value: stats.avgWait !== null ? fmtDuration(stats.avgWait) : "—", color: "var(--tr-red)" }, { label: "Ø Verweildauer", value: stats.avgDwell !== null ? fmtDuration(stats.avgDwell) : "—", color: "var(--tr-warning)" }, { label: "Aktivste Rampe", value: stats.topRampEntry ? `Rampe ${stats.topRampEntry[0]} (${stats.topRampEntry[1]}×)` : "—", color: "var(--tr-text)" }] as s}
                    <div class="flex flex-col gap-0.5">
                        <div
                            class="font-mono text-[10px] uppercase font-medium"
                            style="letter-spacing:1px; color:var(--tr-text-faint);"
                        >
                            {s.label}
                        </div>
                        <div
                            class="font-mono text-[16px] font-semibold leading-none"
                            style="color:{s.color};"
                        >
                            {s.value}
                        </div>
                    </div>
                {/each}
                {#if periodLoading}
                    <div
                        class="flex items-center gap-2 ml-auto text-[12px]"
                        style="color:var(--tr-text-faint);"
                    >
                        <div
                            class="w-4 h-4 rounded-full animate-spin"
                            style="border:2px solid var(--tr-line); border-top-color:var(--tr-green);"
                        ></div>
                        Lade…
                    </div>
                {/if}
            </div>

            <!-- Body: trend + table -->
            <div class="flex-1 overflow-auto chat-scroll flex flex-col">
                <!-- Hour-of-day heatmap (always shown when events exist) -->
                {#if stats.totalEvents > 0}
                    <div
                        class="px-6 py-4 shrink-0"
                        style="border-bottom:1px solid var(--tr-line);"
                    >
                        <div
                            class="font-mono text-[10px] uppercase font-medium mb-3 flex items-center justify-between"
                            style="letter-spacing:1.2px; color:var(--tr-text-faint);"
                        >
                            <span>Aktivität nach Tagesstunde</span>
                            <span style="text-transform:none; letter-spacing:0;">
                                Peak: {hourRows.reduce(
                                    (best, [h, n]) => (n > best[1] ? [h, n] : best),
                                    [0, 0] as [number, number],
                                )[0]}:00 Uhr
                            </span>
                        </div>
                        <div class="flex items-end gap-[3px]" style="height:52px;">
                            {#each hourRows as [hour, count]}
                                {@const pct =
                                    maxHourCount > 0
                                        ? (count / maxHourCount) * 100
                                        : 0}
                                <div
                                    class="flex flex-col items-center gap-1 flex-1 min-w-0"
                                    title="{String(hour).padStart(2, '0')}:00 — {count} Ereignisse"
                                >
                                    <div
                                        class="w-full rounded-t-[2px]"
                                        style="height:{Math.max(
                                            count > 0 ? 4 : 1,
                                            pct * 0.42,
                                        )}px; background:{count > 0
                                            ? 'var(--tr-warning)'
                                            : 'var(--tr-line)'}; opacity:{count >
                                        0
                                            ? 0.85
                                            : 0.4};"
                                    ></div>
                                    <div
                                        class="font-mono text-[8px]"
                                        style="color:var(--tr-text-faint); white-space:nowrap;"
                                    >
                                        {String(hour).padStart(2, "0")}
                                    </div>
                                </div>
                            {/each}
                        </div>
                    </div>
                {/if}

                <!-- Completions per day trend (hidden for Tag view) -->
                {#if statPeriod !== "day" && dayMap.length > 0}
                    <div
                        class="px-6 py-4 shrink-0"
                        style="border-bottom:1px solid var(--tr-line);"
                    >
                        <div
                            class="font-mono text-[10px] uppercase font-medium mb-3"
                            style="letter-spacing:1.2px; color:var(--tr-text-faint);"
                        >
                            Abfertigungen pro Tag
                        </div>
                        <div class="flex items-end gap-1" style="height:52px;">
                            {#each dayMap as [day, count]}
                                {@const pct =
                                    maxDayCount > 0
                                        ? (count / maxDayCount) * 100
                                        : 0}
                                <div
                                    class="flex flex-col items-center gap-1 flex-1 min-w-0"
                                    title="{day}: {count} Abfertigungen"
                                >
                                    <div
                                        class="w-full rounded-t-[2px] flex items-start justify-center"
                                        style="height:{Math.max(
                                            4,
                                            pct * 0.48,
                                        )}px; background:var(--tr-green); opacity:0.75;"
                                    >
                                        {#if dayMap.length <= 14 && count > 0 && pct > 35}
                                            <span
                                                class="font-mono text-[8px] font-semibold mt-[1px]"
                                                style="color:#fff;"
                                                >{count}</span
                                            >
                                        {/if}
                                    </div>
                                    {#if dayMap.length <= 14}
                                        <div
                                            class="font-mono text-[8px]"
                                            style="color:var(--tr-text-faint); white-space:nowrap;"
                                        >
                                            {day.slice(5)}
                                        </div>
                                    {/if}
                                </div>
                            {/each}
                        </div>
                    </div>
                {/if}

                <!-- Top ramps with utilization % (always shown when there are events) -->
                {#if topRamps.length > 0}
                    <div
                        class="px-6 py-4 shrink-0"
                        style="border-bottom:1px solid var(--tr-line);"
                    >
                        <div
                            class="font-mono text-[10px] uppercase font-medium mb-3"
                            style="letter-spacing:1.2px; color:var(--tr-text-faint);"
                        >
                            Aktivität & Auslastung pro Rampe (Top 8)
                        </div>
                        <div class="flex flex-col gap-2">
                            {#each topRamps as [id, count, util]}
                                {@const pct =
                                    stats.byRamp.size > 0
                                        ? (count / (topRamps[0][1] || 1)) * 100
                                        : 0}
                                <div class="flex items-center gap-3">
                                    <div
                                        class="font-mono text-[12px] font-semibold w-14 shrink-0"
                                        style="color:var(--tr-text);"
                                    >
                                        Rampe {id}
                                    </div>
                                    <div
                                        class="flex-1 rounded-full overflow-hidden"
                                        style="height:6px; background:var(--tr-line);"
                                    >
                                        <div
                                            class="h-full rounded-full"
                                            style="width:{pct}%; background:var(--tr-green);"
                                        ></div>
                                    </div>
                                    <div
                                        class="font-mono text-[11px] w-12 text-right"
                                        style="color:var(--tr-text-dim);"
                                    >
                                        {count}×
                                    </div>
                                    <div
                                        class="font-mono text-[11px] font-semibold w-12 text-right"
                                        style="color:{util >= 70
                                            ? 'var(--tr-red)'
                                            : util >= 40
                                              ? 'var(--tr-warning)'
                                              : 'var(--tr-text-dim)'};"
                                        title="Auslastung (Belegt-Zeit ÷ Periodendauer)"
                                    >
                                        {util}%
                                    </div>
                                </div>
                            {/each}
                        </div>
                    </div>
                {/if}

                <!-- Per-employee activity + outliers (side by side) -->
                {#if stats.totalEvents > 0}
                    <div
                        class="grid gap-0 shrink-0"
                        style="grid-template-columns:1fr 1fr; border-bottom:1px solid var(--tr-line);"
                    >
                        <!-- Employee activity -->
                        <div
                            class="px-6 py-4"
                            style="border-right:1px solid var(--tr-line);"
                        >
                            <div
                                class="font-mono text-[10px] uppercase font-medium mb-3"
                                style="letter-spacing:1.2px; color:var(--tr-text-faint);"
                            >
                                Aktivität pro Mitarbeiter
                            </div>
                            {#if userRows.length === 0}
                                <div
                                    class="text-[12px]"
                                    style="color:var(--tr-text-faint);"
                                >
                                    —
                                </div>
                            {:else}
                                {@const maxUser = userRows[0][1]}
                                <div class="flex flex-col gap-1.5">
                                    {#each userRows.slice(0, 8) as [user, count]}
                                        {@const pct = (count / maxUser) * 100}
                                        <div class="flex items-center gap-3">
                                            <div
                                                class="text-[11px] truncate"
                                                style="color:var(--tr-text); width:120px;"
                                                title={user}
                                            >
                                                {user}
                                            </div>
                                            <div
                                                class="flex-1 rounded-full overflow-hidden"
                                                style="height:5px; background:var(--tr-line);"
                                            >
                                                <div
                                                    class="h-full rounded-full"
                                                    style="width:{pct}%; background:var(--tr-text-dim);"
                                                ></div>
                                            </div>
                                            <div
                                                class="font-mono text-[11px] w-8 text-right"
                                                style="color:var(--tr-text-dim);"
                                            >
                                                {count}
                                            </div>
                                        </div>
                                    {/each}
                                </div>
                            {/if}
                        </div>

                        <!-- Outliers: longest waits + longest occupancies -->
                        <div class="px-6 py-4 flex flex-col gap-3">
                            <div>
                                <div
                                    class="font-mono text-[10px] uppercase font-medium mb-2"
                                    style="letter-spacing:1.2px; color:var(--tr-text-faint);"
                                >
                                    Längste Wartezeiten
                                </div>
                                {#if stats.longestWaits.length === 0}
                                    <div
                                        class="text-[12px]"
                                        style="color:var(--tr-text-faint);"
                                    >
                                        —
                                    </div>
                                {:else}
                                    <div class="flex flex-col gap-1">
                                        {#each stats.longestWaits as ev}
                                            <div
                                                class="flex items-center gap-3 text-[11px]"
                                            >
                                                <div
                                                    class="font-mono font-semibold w-14 shrink-0"
                                                    style="color:var(--tr-red);"
                                                >
                                                    {fmtDuration(
                                                        ev.duration_min,
                                                    )}
                                                </div>
                                                <div
                                                    class="font-mono w-14 shrink-0"
                                                    style="color:var(--tr-text);"
                                                >
                                                    R {ev.ramp_id}
                                                </div>
                                                <div
                                                    class="font-mono shrink-0"
                                                    style="color:var(--tr-text-dim); width:90px;"
                                                >
                                                    {ev.kennzeichen || "—"}
                                                </div>
                                                <div
                                                    class="truncate"
                                                    style="color:var(--tr-text-faint);"
                                                    title={ev.user}
                                                >
                                                    {ev.user}
                                                </div>
                                            </div>
                                        {/each}
                                    </div>
                                {/if}
                            </div>
                            <div>
                                <div
                                    class="font-mono text-[10px] uppercase font-medium mb-2"
                                    style="letter-spacing:1.2px; color:var(--tr-text-faint);"
                                >
                                    Längste Belegungen
                                </div>
                                {#if stats.longestOccupancies.length === 0}
                                    <div
                                        class="text-[12px]"
                                        style="color:var(--tr-text-faint);"
                                    >
                                        —
                                    </div>
                                {:else}
                                    <div class="flex flex-col gap-1">
                                        {#each stats.longestOccupancies as ev}
                                            <div
                                                class="flex items-center gap-3 text-[11px]"
                                            >
                                                <div
                                                    class="font-mono font-semibold w-14 shrink-0"
                                                    style="color:var(--tr-warning);"
                                                >
                                                    {fmtDuration(
                                                        ev.duration_min,
                                                    )}
                                                </div>
                                                <div
                                                    class="font-mono w-14 shrink-0"
                                                    style="color:var(--tr-text);"
                                                >
                                                    R {ev.ramp_id}
                                                </div>
                                                <div
                                                    class="font-mono shrink-0"
                                                    style="color:var(--tr-text-dim); width:90px;"
                                                >
                                                    {ev.kennzeichen || "—"}
                                                </div>
                                                <div
                                                    class="truncate"
                                                    style="color:var(--tr-text-faint);"
                                                    title={ev.user}
                                                >
                                                    {ev.user}
                                                </div>
                                            </div>
                                        {/each}
                                    </div>
                                {/if}
                            </div>
                        </div>
                    </div>
                {/if}

                <!-- Event table (always shown) -->
                {#if dayEntries.length === 0}
                    <div
                        class="flex flex-col items-center justify-center py-16 gap-3"
                        style="color:var(--tr-text-faint);"
                    >
                        <svg
                            width="28"
                            height="28"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="1.5"
                            ><path
                                d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"
                            /><polyline points="14 2 14 8 20 8" /></svg
                        >
                        <p class="text-sm font-medium">
                            Keine Ereignisse im gewählten Zeitraum
                        </p>
                    </div>
                {:else}
                    <table class="w-full text-left border-collapse">
                        <thead>
                            <tr
                                style="position:sticky; top:0; background:var(--tr-surface2); border-bottom:1px solid var(--tr-line);"
                            >
                                {#each statPeriod === "day" ? ["Zeit", "Rampe", "Vorher → Nachher", "Dauer", "Mitarbeiter", "Kennzeichen"] : ["Datum", "Zeit", "Rampe", "Vorher → Nachher", "Dauer", "Mitarbeiter", "Kennzeichen"] as col}
                                    <th
                                        class="font-mono text-[10px] font-semibold uppercase px-4 py-2.5"
                                        style="letter-spacing:1px; color:var(--tr-text-faint); white-space:nowrap;"
                                        >{col}</th
                                    >
                                {/each}
                            </tr>
                        </thead>
                        <tbody>
                            {#each dayEntries as ev, i}
                                {@const tone = getStatusTone(ev.to_status)}
                                <tr
                                    style="border-bottom:1px solid var(--tr-line); background:{i %
                                        2 ===
                                    0
                                        ? 'transparent'
                                        : 'rgba(0,0,0,0.025)'};"
                                >
                                    {#if statPeriod !== "day"}
                                        <td
                                            class="px-4 py-2 font-mono text-[11px]"
                                            style="color:var(--tr-text-dim); white-space:nowrap;"
                                        >
                                            {ev.timestamp.slice(0, 10)}
                                        </td>
                                    {/if}
                                    <td
                                        class="px-4 py-2 font-mono text-[11px]"
                                        style="color:var(--tr-text-dim); white-space:nowrap;"
                                    >
                                        {new Date(
                                            ev.timestamp,
                                        ).toLocaleTimeString("de-DE", {
                                            hour: "2-digit",
                                            minute: "2-digit",
                                            second: "2-digit",
                                        })}
                                    </td>
                                    <td class="px-4 py-2">
                                        <span
                                            class="font-mono text-[14px] font-semibold"
                                            style="color:var(--tr-text);"
                                            >{ev.ramp_id}</span
                                        >
                                    </td>
                                    <td class="px-4 py-2">
                                        <div class="flex items-center gap-1.5">
                                            <span
                                                class="font-mono px-1.5 py-0.5 rounded text-[9px] font-semibold uppercase"
                                                style="background:{getStatusTone(
                                                    ev.from_status,
                                                ).bg}; color:{getStatusTone(
                                                    ev.from_status,
                                                )
                                                    .fg}; border:1px solid {getStatusTone(
                                                    ev.from_status,
                                                ).line};"
                                            >
                                                {fmtStatusDE(ev.from_status)}
                                            </span>
                                            <svg
                                                width="9"
                                                height="9"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="2.5"
                                                style="color:var(--tr-text-faint);"
                                                ><path
                                                    d="M5 12h14M12 5l7 7-7 7"
                                                /></svg
                                            >
                                            <span
                                                class="font-mono px-1.5 py-0.5 rounded text-[9px] font-semibold uppercase"
                                                style="background:{tone.bg}; color:{tone.fg}; border:1px solid {tone.line};"
                                            >
                                                {fmtStatusDE(ev.to_status)}
                                            </span>
                                        </div>
                                    </td>
                                    <td
                                        class="px-4 py-2 font-mono text-[11px]"
                                        style="color:var(--tr-text-dim);"
                                        >{fmtDuration(ev.duration_min)}</td
                                    >
                                    <td
                                        class="px-4 py-2 text-[11px]"
                                        style="color:var(--tr-text-dim);"
                                        >{ev.user}</td
                                    >
                                    <td
                                        class="px-4 py-2 font-mono text-[11px]"
                                        style="color:{ev.kennzeichen
                                            ? 'var(--tr-text)'
                                            : 'var(--tr-text-faint)'};"
                                    >
                                        {ev.kennzeichen || "—"}
                                    </td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                {/if}
            </div>
        </div>
    </div>
{/if}

<!-- ── Benutzerverwaltung Modal ── -->
{#if showUserMgmt}
    <div
        class="fixed inset-0 z-50 flex items-center justify-center"
        style="background: rgba(0,0,0,0.6); backdrop-filter: blur(4px);"
        onclick={(e) => {
            if (e.target === e.currentTarget) showUserMgmt = false;
        }}
        onkeydown={(e) => {
            if (e.key === "Escape") showUserMgmt = false;
        }}
        role="dialog"
        tabindex="-1"
        aria-modal="true"
        aria-label="Benutzerverwaltung"
    >
        <div
            class="flex flex-col rounded-2xl overflow-hidden shadow-2xl"
            style="width: 520px; max-width: 96vw; max-height: 80vh; background: var(--tr-surface); border: 1px solid var(--tr-line);"
        >
            <!-- Header -->
            <div
                class="flex items-center gap-4 px-6 py-4 shrink-0"
                style="border-bottom: 1px solid var(--tr-line); background: var(--tr-always-dark);"
            >
                <div class="flex-1">
                    <div class="text-[15px] font-semibold" style="color:#fff;">
                        Benutzerverwaltung
                    </div>
                    <div
                        class="font-mono text-[10.5px] uppercase mt-[3px]"
                        style="letter-spacing:1.4px; color:rgba(255,255,255,0.45);"
                    >
                        {allUsers.length} Benutzer registriert
                    </div>
                </div>
                <button
                    onclick={() => (showUserMgmt = false)}
                    aria-label="Schließen"
                    class="w-8 h-8 rounded-lg grid place-items-center cursor-pointer"
                    style="border:1px solid rgba(255,255,255,0.1); background:transparent; color:rgba(255,255,255,0.6);"
                >
                    <svg
                        width="13"
                        height="13"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"><path d="M18 6L6 18M6 6l12 12" /></svg
                    >
                </button>
            </div>

            <!-- User list -->
            <div class="flex-1 overflow-auto chat-scroll p-4">
                {#if userMgmtLoading}
                    <div
                        class="flex items-center justify-center py-12 gap-3"
                        style="color:var(--tr-text-faint);"
                    >
                        <div
                            class="w-5 h-5 rounded-full animate-spin"
                            style="border:2px solid var(--tr-line); border-top-color:var(--tr-green);"
                        ></div>
                        Lade…
                    </div>
                {:else if allUsers.length === 0}
                    <p
                        class="text-center py-12 text-sm"
                        style="color:var(--tr-text-faint);"
                    >
                        Keine Benutzer gefunden.
                    </p>
                {:else}
                    <table class="w-full border-collapse">
                        <thead>
                            <tr style="border-bottom:1px solid var(--tr-line);">
                                <th
                                    class="text-left font-mono text-[10px] uppercase pb-2.5 px-2"
                                    style="letter-spacing:1px; color:var(--tr-text-faint);"
                                    >Benutzer</th
                                >
                                <th
                                    class="text-left font-mono text-[10px] uppercase pb-2.5 px-2"
                                    style="letter-spacing:1px; color:var(--tr-text-faint);"
                                    >Rolle</th
                                >
                            </tr>
                        </thead>
                        <tbody>
                            {#each allUsers as u (u.username)}
                                <tr
                                    style="border-bottom:1px solid var(--tr-line);"
                                >
                                    <td class="py-2.5 px-2">
                                        <div class="flex items-center gap-2">
                                            <div
                                                class="w-6 h-6 rounded-full grid place-items-center text-[10px] font-semibold shrink-0"
                                                style="background:var(--tr-red); color:#fff;"
                                            >
                                                {u.username
                                                    .charAt(0)
                                                    .toUpperCase()}
                                            </div>
                                            <span
                                                class="text-[13px]"
                                                style="color:var(--tr-text);"
                                                >{u.username}</span
                                            >
                                            {#if u.username === currentUser}
                                                <span
                                                    class="font-mono text-[9px] px-1.5 py-0.5 rounded"
                                                    style="background:var(--tr-surface2); color:var(--tr-text-faint); border:1px solid var(--tr-line);"
                                                    >Ich</span
                                                >
                                            {/if}
                                        </div>
                                    </td>
                                    <td class="py-2.5 px-2">
                                        <select
                                            value={u.role}
                                            onchange={(e) =>
                                                changeUserRole(
                                                    u.username,
                                                    e.currentTarget.value,
                                                )}
                                            class="rounded-[6px] px-2 py-1.5 text-[12px] outline-none cursor-pointer"
                                            style="background:var(--tr-surface2); border:1px solid var(--tr-line); color:var(--tr-text);"
                                        >
                                            <option value="member_lager"
                                                >Lager (Standard)</option
                                            >
                                            <option value="member_buero"
                                                >Büro</option
                                            >
                                            <option value="admin"
                                                >Administrator</option
                                            >
                                        </select>
                                    </td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                {/if}
            </div>

            <!-- Footer -->
            <div
                class="shrink-0 px-4 py-3 text-[11px]"
                style="border-top:1px solid var(--tr-line); color:var(--tr-text-faint); background:var(--tr-surface2);"
            >
                Neue Benutzer erhalten automatisch die Rolle "Lager" wenn sie
                die App zum ersten Mal öffnen.
            </div>
        </div>
    </div>
{/if}

<!-- ── Pending-Status Dialog (Büro/Admin: Frei → Wartend) ── -->
{#if pendingDialog}
    <div
        class="fixed inset-0 z-50 flex items-center justify-center"
        style="background: rgba(0,0,0,0.6); backdrop-filter: blur(4px);"
        onclick={(e) => {
            if (e.target === e.currentTarget) cancelPendingDialog();
        }}
        onkeydown={(e) => {
            if (e.key === "Escape") cancelPendingDialog();
        }}
        role="dialog"
        tabindex="-1"
        aria-modal="true"
        aria-label="LKW & Reservierung"
    >
        <div
            class="flex flex-col rounded-2xl overflow-hidden shadow-2xl"
            style="width: 460px; max-width: 96vw; background: var(--tr-surface); border: 1px solid var(--tr-line);"
        >
            <!-- Header -->
            <div
                class="flex items-center gap-4 px-6 py-4 shrink-0"
                style="border-bottom: 1px solid var(--tr-line); background: var(--tr-always-dark);"
            >
                <div class="flex-1">
                    <div class="text-[15px] font-semibold" style="color:#fff;">
                        Rampe {pendingDialog.ramp.id} · Reservierung
                    </div>
                    <div
                        class="font-mono text-[10.5px] uppercase mt-[3px]"
                        style="letter-spacing:1.4px; color:rgba(255,255,255,0.45);"
                    >
                        Frei → Wartend
                    </div>
                </div>
                <button
                    onclick={cancelPendingDialog}
                    aria-label="Schließen"
                    class="w-8 h-8 rounded-lg grid place-items-center cursor-pointer"
                    style="border:1px solid rgba(255,255,255,0.1); background:transparent; color:rgba(255,255,255,0.6);"
                >
                    <svg
                        width="13"
                        height="13"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        ><path d="M18 6L6 18M6 6l12 12" /></svg
                    >
                </button>
            </div>

            <!-- Body -->
            <div class="flex flex-col gap-4 px-6 py-5">
                <label class="flex flex-col gap-1.5">
                    <span
                        class="font-mono text-[10px] font-semibold uppercase"
                        style="letter-spacing:1.2px; color:var(--tr-text-faint);"
                    >
                        LKW
                    </span>
                    <input
                        type="text"
                        class="w-full font-mono text-[13px] px-3 py-2 rounded-md outline-none"
                        style="background:var(--tr-surface2); color:var(--tr-text); border:1px solid var(--tr-line); letter-spacing:0.5px;"
                        maxlength={15}
                        placeholder="z.B. M-TR 2418"
                        value={pendingDialog.kennzeichen}
                        oninput={(e) => {
                            if (pendingDialog)
                                pendingDialog.kennzeichen =
                                    e.currentTarget.value;
                        }}
                        onkeydown={(e) => {
                            if (e.key === "Enter") {
                                e.preventDefault();
                                confirmPendingDialog();
                            }
                        }}
                        use:focusInput
                    />
                </label>
                <label class="flex flex-col gap-1.5">
                    <span
                        class="font-mono text-[10px] font-semibold uppercase"
                        style="letter-spacing:1.2px; color:var(--tr-text-faint);"
                    >
                        RES
                    </span>
                    <input
                        type="text"
                        class="w-full text-[13px] px-3 py-2 rounded-md outline-none"
                        style="background:var(--tr-surface2); color:var(--tr-text); border:1px solid var(--tr-line);"
                        maxlength={40}
                        placeholder="Reserviert für…"
                        value={pendingDialog.reserviert_fuer}
                        oninput={(e) => {
                            if (pendingDialog)
                                pendingDialog.reserviert_fuer =
                                    e.currentTarget.value;
                        }}
                        onkeydown={(e) => {
                            if (e.key === "Enter") {
                                e.preventDefault();
                                confirmPendingDialog();
                            }
                        }}
                    />
                </label>
            </div>

            <!-- Footer -->
            <div
                class="flex items-center justify-end gap-2 px-6 py-4"
                style="border-top:1px solid var(--tr-line); background:var(--tr-surface2);"
            >
                <button
                    onclick={cancelPendingDialog}
                    class="px-4 h-9 rounded-lg text-[13px] font-medium cursor-pointer"
                    style="border:1px solid var(--tr-line); background:transparent; color:var(--tr-text);"
                >
                    Abbrechen
                </button>
                <button
                    onclick={confirmPendingDialog}
                    class="px-4 h-9 rounded-lg text-[13px] font-semibold cursor-pointer"
                    style="border:1px solid var(--tr-warning); background:var(--tr-warning); color:#000;"
                >
                    Bestätigen
                </button>
            </div>
        </div>
    </div>
{/if}

<style>
    @keyframes mc-pulse {
        0%,
        100% {
            opacity: 1;
        }
        50% {
            opacity: 0.35;
        }
    }

    .ramp-tile:hover {
        transform: scale(1.02) translateY(-1px);
        box-shadow: 0 8px 24px -8px rgba(0, 0, 0, 0.25);
    }
    .ramp-tile:active {
        transform: scale(0.98);
    }

    .chat-panel {
        transition: width 0.25s cubic-bezier(0.4, 0, 0.2, 1);
    }

    .chat-scroll {
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 0, 0, 0.15) transparent;
    }
    :global(.dark) .chat-scroll {
        scrollbar-color: rgba(255, 255, 255, 0.15) transparent;
    }
    .chat-scroll::-webkit-scrollbar {
        width: 4px;
    }
    .chat-scroll::-webkit-scrollbar-track {
        background: transparent;
    }
    .chat-scroll::-webkit-scrollbar-thumb {
        background: rgba(0, 0, 0, 0.15);
        border-radius: 2px;
    }
    :global(.dark) .chat-scroll::-webkit-scrollbar-thumb {
        background: rgba(255, 255, 255, 0.15);
    }
</style>
