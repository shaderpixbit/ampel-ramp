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
    } from "$lib/ramp-utils";
    import { LoaderCircleIcon } from "@lucide/svelte";

    interface ChatMessage {
        id: string;
        user: string;
        text: string;
        timestamp: string;
    }

    let ramps = $state<Ramp[]>([]);
    let currentUser = $state("Unknown User");
    let syncTimeout: number;
    let isUpdating = $state(false);
    let isConnected = $state(true);
    let isFetching = false;
    let syncError = $state<string | null>(null);
    let errorTimeout: number;

    let messages = $state<ChatMessage[]>([]);
    let chatInput = $state("");
    let isSendingMessage = $state(false);
    let chatScrollEl: HTMLElement;
    let chatOpen = $state(false);
    let lastReadCount = $state(0);
    let unreadCount = $derived(chatOpen ? 0 : Math.max(0, messages.length - lastReadCount));

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
        errorTimeout = window.setTimeout(() => { syncError = null; }, 4000);
    }

    onMount(async () => {
        try {
            currentUser = await invoke("get_current_user");
        } catch (e) {
            console.error("Failed to get current user:", e);
        }

        await fetchRamps();

        try {
            messages = await invoke("get_messages");
            lastReadCount = messages.length;
        } catch (e) {
            console.error("Failed to load messages:", e);
        }

        clockInterval = window.setInterval(() => { now = Date.now(); }, 1000);
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
                const [dbRamps, dbMessages] = await Promise.all([
                    invoke<Ramp[]>("get_ramps"),
                    invoke<ChatMessage[]>("get_messages"),
                ]);

                if (JSON.stringify(ramps) !== JSON.stringify(dbRamps)) ramps = dbRamps;

                if (JSON.stringify(messages) !== JSON.stringify(dbMessages)) {
                    messages = dbMessages;
                    if (chatOpen) {
                        lastReadCount = messages.length;
                        await tick();
                        if (chatScrollEl) chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
                    }
                }

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
        const newStatus = cycleRampStatus(ramp.status);
        const now_iso = new Date().toISOString();
        const updatedRamp: Ramp = { ...ramp, status: newStatus, last_updated_by: currentUser, last_updated_at: now_iso };
        const index = ramps.findIndex((r) => r.id === ramp.id);
        const oldRamp = index !== -1 ? { ...ramps[index] } : null;
        if (index !== -1) ramps[index] = updatedRamp;
        isUpdating = true;
        try {
            const updatedRamps: Ramp[] = await invoke("update_ramp", { updatedRamp });
            ramps = updatedRamps;
        } catch (e) {
            console.error("Failed to update ramp:", e);
            if (index !== -1 && oldRamp) ramps[index] = oldRamp;
            showError(`Rampe ${ramp.id} konnte nicht aktualisiert werden.`);
        } finally {
            isUpdating = false;
        }
    }

    async function sendMessage() {
        const text = chatInput.trim();
        if (!text || isSendingMessage) return;
        chatInput = "";
        isSendingMessage = true;
        try {
            const updated: ChatMessage[] = await invoke("send_message", { user: currentUser, text });
            messages = updated;
            lastReadCount = messages.length;
            await tick();
            if (chatScrollEl) chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
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

    function closeChat() { chatOpen = false; }

    function getDwellTime(ramp: Ramp): string {
        if (!ramp.last_updated_at || ramp.status === "free") return "00:00";
        const ms = now - new Date(ramp.last_updated_at).getTime();
        const totalMin = Math.max(0, Math.floor(ms / 60000));
        const h = Math.floor(totalMin / 60);
        const m = totalMin % 60;
        return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
    }

    function getStatusTone(status: string) {
        if (status === "free")    return { fg: "var(--tr-green)",   bg: "var(--tr-green-bg)",   line: "var(--tr-green-line)",   glow: "var(--tr-green-glow)",   label: "Frei" };
        if (status === "pending") return { fg: "var(--tr-warning)", bg: "var(--tr-warning-bg)", line: "var(--tr-warning-line)", glow: "var(--tr-warning-glow)", label: "Wartend" };
        return                      { fg: "var(--tr-red)",     bg: "var(--tr-red-bg)",     line: "var(--tr-red-line)",     glow: "var(--tr-red-glow)",     label: "Belegt" };
    }

    function userInitials(user: string): string {
        return user.split(/[.\s_-]/).filter(Boolean).map(p => p[0]?.toUpperCase() ?? "").slice(0, 2).join("") || "?";
    }

    let utilization = $derived(ramps.length > 0 ? Math.round((counts.pending + counts.closed) / ramps.length * 100) : 0);
</script>

<!-- Error toast -->
{#if syncError}
    <div class="fixed top-4 left-1/2 -translate-x-1/2 z-50 text-white text-sm font-medium px-4 py-2 rounded-lg shadow-xl"
         style="background: var(--tr-red);">
        {syncError}
    </div>
{/if}

<div class="h-screen w-screen overflow-hidden flex flex-col" style="background: var(--tr-bg); color: var(--tr-text); font-family: 'Inter Variable', sans-serif;">

    <!-- ── Header ── -->
    <header class="flex items-center gap-4 px-5 shrink-0"
            style="height: 60px; background: var(--tr-always-dark); border-bottom: 1px solid var(--tr-line);">

        <!-- Logo + Title -->
        <div class="flex items-center gap-3">
            <div class="w-8 h-8 rounded-md flex items-center justify-center font-bold text-sm"
                 style="background: var(--tr-red); color: #fff;">T</div>
            <div>
                <div class="text-[15px] font-semibold tracking-[-0.2px] leading-none" style="color: #fff;">
                    Troiber<span class="font-normal ml-2" style="color: rgba(255,255,255,0.5);">/ Ramp Control</span>
                </div>
                <div class="font-mono text-[10.5px] uppercase tracking-[1.4px] mt-[3px]"
                     style="color: rgba(255,255,255,0.45);">Werk Nürnberg · Sektor A</div>
            </div>
        </div>

        <div class="flex-1"></div>

        <!-- Sync pill -->
        <div class="flex items-center gap-2 px-3 py-1.5 rounded-full font-mono text-[11px] tracking-[0.6px]"
             style="background: rgba(255,255,255,0.06); border: 1px solid rgba(255,255,255,0.08); color: rgba(255,255,255,0.7);">
            <span class="w-[7px] h-[7px] rounded-full"
                  class:opacity-50={!isConnected}
                  style="background: var(--tr-green); box-shadow: 0 0 0 3px {isConnected ? 'rgba(104,197,47,0.15)' : 'transparent'}, 0 0 8px {isConnected ? 'rgba(104,197,47,0.6)' : 'transparent'};"></span>
            {isConnected ? "LIVE · 1.5s" : "OFFLINE"}
        </div>

        <!-- Status chips -->
        <div class="flex gap-1.5">
            {#each [
                { status: "free",    label: "Frei",    count: counts.free },
                { status: "pending", label: "Wartend", count: counts.pending },
                { status: "closed",  label: "Belegt",  count: counts.closed },
            ] as chip}
                {@const tone = getStatusTone(chip.status)}
                <div class="flex items-baseline gap-2 px-3 py-1.5 rounded-lg font-mono min-w-[82px]"
                     style="background: {tone.bg}; border: 1px solid {tone.line};">
                    <span class="text-[18px] font-semibold leading-none tabular-nums" style="color: {tone.fg};">
                        {String(chip.count).padStart(2, "0")}
                    </span>
                    <span class="text-[11px] font-semibold uppercase tracking-[0.6px]" style="color: {tone.fg};">
                        {chip.label}
                    </span>
                </div>
            {/each}
        </div>

        <div class="w-px h-6 mx-1" style="background: rgba(255,255,255,0.1);"></div>

        <!-- Theme toggle -->
        <button onclick={toggleMode}
                class="w-[34px] h-[34px] rounded-[10px] grid place-items-center cursor-pointer transition-colors"
                style="border: 1px solid rgba(255,255,255,0.1); background: rgba(255,255,255,0.04); color: rgba(255,255,255,0.7);"
                aria-label="Theme wechseln">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" class="dark:hidden"><circle cx="12" cy="12" r="4"/><path d="M12 3v1M12 20v1M4.93 4.93l.71.71M18.36 18.36l.71.71M3 12h1M20 12h1M4.93 19.07l.71-.71M18.36 5.64l.71-.71"/></svg>
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="hidden dark:block"><path d="M20.5 14.2A8 8 0 1 1 9.8 3.5a6 6 0 0 0 10.7 10.7z"/></svg>
        </button>

        <!-- Chat toggle -->
        <button onclick={chatOpen ? closeChat : openChat}
                class="relative h-[34px] px-4 rounded-[10px] flex items-center gap-2 text-[13px] font-medium cursor-pointer transition-colors"
                style="border: 1px solid {chatOpen ? 'var(--tr-red)' : 'rgba(255,255,255,0.1)'}; background: {chatOpen ? 'rgba(218,43,41,0.15)' : 'rgba(255,255,255,0.04)'}; color: {chatOpen ? '#FF6B69' : 'rgba(255,255,255,0.9)'};">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
            Team
            {#if unreadCount > 0}
                <span class="min-w-[18px] h-[18px] px-[5px] rounded-full grid place-items-center text-[10px] font-semibold font-mono"
                      style="background: var(--tr-red); color: #fff;">{unreadCount}</span>
            {/if}
        </button>

        <!-- User chip -->
        <div class="flex items-center gap-2.5 pl-1 pr-3 rounded-full"
             style="background: rgba(255,255,255,0.04); border: 1px solid rgba(255,255,255,0.08);">
            <div class="w-[26px] h-[26px] rounded-full grid place-items-center text-[11.5px] font-semibold"
                 style="background: var(--tr-red); color: #fff;">{userInitials(currentUser)}</div>
            <div class="leading-[1.15]">
                <div class="text-[12.5px] font-medium" style="color: rgba(255,255,255,0.9);">{currentUser}</div>
                <div class="font-mono text-[10px] uppercase tracking-[0.8px] mt-[1px]"
                     style="color: rgba(255,255,255,0.45);">Operator</div>
            </div>
        </div>
    </header>

    <!-- ── Body ── -->
    <div class="flex-1 overflow-hidden relative">

        <!-- Main scroll area -->
        <main class="h-full overflow-auto" style="padding: 22px 26px;">

            <!-- Section header -->
            <div class="flex items-end justify-between mb-5 gap-6">
                <div>
                    <div class="flex items-center gap-2.5 mb-2">
                        <span class="font-mono text-[11px] uppercase tracking-[1.6px] font-medium"
                              style="color: var(--tr-text-faint);">01 / Sektor A · Tor 30–42</span>
                        <span class="w-4 h-px" style="background: var(--tr-line);"></span>
                        <span class="font-mono text-[11px] uppercase tracking-[1.6px] font-medium"
                              style="color: var(--tr-text-faint);">13 Rampen</span>
                    </div>
                    <h1 class="text-[24px] font-semibold tracking-[-0.4px] m-0">Rampenbelegung</h1>
                </div>
                <div class="flex items-center gap-2 text-[11.5px]" style="color: var(--tr-text-faint);">
                    <kbd class="px-[7px] py-[2px] rounded-[5px] font-mono text-[10.5px]"
                         style="border: 1px solid var(--tr-line); color: var(--tr-text-dim); background: var(--tr-surface);">Klick</kbd>
                    um Status zu ändern
                </div>
            </div>

            <!-- Ramp grid -->
            {#if ramps.length === 0}
                <div class="flex flex-col items-center justify-center py-24 gap-3" style="color: var(--tr-text-faint);">
                    <div class="w-8 h-8 border-2 border-t-[color:var(--tr-green)] rounded-full animate-spin"
                         style="border-color: var(--tr-line); border-top-color: var(--tr-green);"></div>
                    <p class="text-sm font-medium">Verbinde…</p>
                </div>
            {:else}
                <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 12px;">
                    {#each ramps as ramp (ramp.id)}
                        {@const locked = isRampLocked(ramp)}
                        {@const tone = getStatusTone(ramp.status)}
                        {@const isFree = ramp.status === "free"}
                        {@const dwell = getDwellTime(ramp)}
                        {@const dt = ramp.last_updated_at ? new Date(ramp.last_updated_at) : null}
                        {@const timeStr = dt ? dt.toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit" }) : null}
                        <button
                            onclick={() => cycleStatus(ramp)}
                            disabled={!isConnected || isUpdating || locked}
                            aria-label="Rampe {ramp.id}, {getStatusLabel(ramp.status)}. Klicken um zu ändern."
                            class="ramp-tile relative rounded-[12px] overflow-hidden text-left flex flex-col gap-[10px] cursor-pointer transition-[transform,box-shadow] duration-150 disabled:cursor-not-allowed"
                            style="
                                padding: 14px 14px 12px;
                                min-height: 152px;
                                background: {isFree ? 'var(--tr-surface)' : 'linear-gradient(180deg, ' + tone.bg + ' 0%, var(--tr-surface) 75%)'};
                                border: 1px solid {isFree ? 'var(--tr-line)' : tone.line};
                                box-shadow: {isFree ? 'none' : '0 0 24px -10px ' + tone.glow};
                            ">

                            <!-- Lock overlay -->
                            {#if locked}
                                <div class="absolute inset-0 z-30 flex items-center justify-center backdrop-blur-[1px]"
                                     style="background: rgba(0,0,0,0.3);">
                                    <LoaderCircleIcon class="w-6 h-6 text-white drop-shadow-md animate-spin" />
                                </div>
                            {/if}

                            <!-- Top accent stripe -->
                            <div class="absolute top-0 left-0 right-0 h-[3px] rounded-t-[12px]"
                                 style="background: {tone.fg}; opacity: {isFree ? 0.35 : 1};"></div>

                            <!-- Row 1: number + status badge -->
                            <div class="flex items-start justify-between gap-2">
                                <div class="font-semibold leading-none tabular-nums"
                                     style="font-size: 36px; letter-spacing: -1.2px; color: var(--tr-text);">{ramp.id}</div>
                                <span class="inline-flex items-center gap-1.5 px-2 py-1 rounded-[6px] font-mono text-[10px] font-semibold uppercase tracking-[0.6px]"
                                      style="background: {tone.bg}; border: 1px solid {tone.line}; color: {tone.fg};">
                                    <span class="w-[6px] h-[6px] rounded-full"
                                          style="background: {tone.fg}; box-shadow: {!isFree ? '0 0 6px ' + tone.glow : 'none'}; animation: {ramp.status === 'pending' ? 'mc-pulse 1.6s ease-in-out infinite' : 'none'};"></span>
                                    {tone.label}
                                </span>
                            </div>

                            <!-- Row 2: LKW plate / operator -->
                            <div class="font-mono text-[13px] font-medium flex items-center gap-2 rounded-[5px] px-[9px] py-[5px]"
                                 style="
                                     color: {ramp.last_updated_by && ramp.last_updated_by !== 'Unknown User' ? 'var(--tr-text)' : 'var(--tr-text-faint)'};
                                     background: var(--tr-surface2);
                                     border: 1px solid var(--tr-line);
                                     letter-spacing: 0.6px;
                                 ">
                                <span class="text-[9px] font-semibold uppercase tracking-[1px]"
                                      style="color: var(--tr-text-faint);">OP</span>
                                <span>{ramp.last_updated_by && ramp.last_updated_by !== "Unknown User" ? ramp.last_updated_by : "— — —"}</span>
                            </div>

                            <!-- Row 3: dwell or last-updated -->
                            <div class="mt-auto flex justify-between items-end">
                                {#if !isFree}
                                    <div>
                                        <div class="font-mono text-[9.5px] uppercase tracking-[1px] mb-1"
                                             style="color: var(--tr-text-faint);">Verweildauer</div>
                                        <div class="font-mono text-[18px] font-semibold leading-none tabular-nums"
                                             style="color: {tone.fg};">{dwell}</div>
                                    </div>
                                    <div class="text-right">
                                        <div class="font-mono text-[9.5px] uppercase tracking-[1px] mb-1"
                                             style="color: var(--tr-text-faint);">Seit</div>
                                        <div class="text-[12px] font-medium" style="color: var(--tr-text-dim);">
                                            {timeStr ?? "—"}
                                        </div>
                                    </div>
                                {:else}
                                    <div class="font-mono text-[11px]" style="color: var(--tr-text-faint); letter-spacing: 0.4px;">
                                        {timeStr ? "Frei seit " + timeStr : "Noch nie belegt"}
                                    </div>
                                {/if}
                            </div>
                        </button>
                    {/each}
                </div>
            {/if}

            <!-- Stats strip -->
            {#if ramps.length > 0}
                <div class="mt-5 rounded-[12px] grid gap-7"
                     style="padding: 16px 20px; background: var(--tr-surface); border: 1px solid var(--tr-line); grid-template-columns: 1fr 1fr 1fr 1fr;">
                    <div class="flex flex-col gap-1.5">
                        <div class="font-mono text-[10px] uppercase tracking-[1.2px] font-medium" style="color: var(--tr-text-faint);">Frei</div>
                        <div class="flex items-baseline gap-1.5">
                            <span class="font-mono text-[22px] font-semibold leading-none tabular-nums" style="color: var(--tr-text);">{String(counts.free).padStart(2, "0")}</span>
                            <span class="text-[10px] uppercase tracking-[0.6px]" style="color: var(--tr-text-faint);">Rampen</span>
                        </div>
                    </div>
                    <div class="flex flex-col gap-1.5">
                        <div class="font-mono text-[10px] uppercase tracking-[1.2px] font-medium" style="color: var(--tr-text-faint);">Wartend</div>
                        <div class="flex items-baseline gap-1.5">
                            <span class="font-mono text-[22px] font-semibold leading-none tabular-nums" style="color: var(--tr-warning);">{String(counts.pending).padStart(2, "0")}</span>
                            <span class="text-[10px] uppercase tracking-[0.6px]" style="color: var(--tr-text-faint);">LKW</span>
                        </div>
                    </div>
                    <div class="flex flex-col gap-1.5">
                        <div class="font-mono text-[10px] uppercase tracking-[1.2px] font-medium" style="color: var(--tr-text-faint);">Belegt</div>
                        <div class="flex items-baseline gap-1.5">
                            <span class="font-mono text-[22px] font-semibold leading-none tabular-nums" style="color: var(--tr-red);">{String(counts.closed).padStart(2, "0")}</span>
                            <span class="text-[10px] uppercase tracking-[0.6px]" style="color: var(--tr-text-faint);">LKW</span>
                        </div>
                    </div>
                    <div class="flex flex-col gap-1.5">
                        <div class="font-mono text-[10px] uppercase tracking-[1.2px] font-medium" style="color: var(--tr-text-faint);">Auslastung</div>
                        <div class="flex items-baseline gap-1.5">
                            <span class="font-mono text-[22px] font-semibold leading-none tabular-nums" style="color: var(--tr-text);">{utilization}%</span>
                            <span class="text-[10px] uppercase tracking-[0.6px]" style="color: var(--tr-text-faint);">{counts.pending + counts.closed} / {ramps.length}</span>
                        </div>
                    </div>
                </div>
            {/if}
        </main>

        <!-- ── Chat panel ── -->
        <aside class="absolute top-0 right-0 bottom-0 overflow-hidden flex flex-col chat-panel"
               style="width: {chatOpen ? '360px' : '0px'}; background: var(--tr-surface); border-left: 1px solid {chatOpen ? 'var(--tr-line)' : 'transparent'};">

            <!-- Chat header -->
            <div class="flex items-start gap-2.5 px-5 py-[18px] shrink-0"
                 style="border-bottom: 1px solid var(--tr-line);">
                <div class="flex-1 min-w-0">
                    <div class="text-[14px] font-semibold tracking-[-0.1px]" style="color: var(--tr-text);">Team Chat</div>
                    <div class="font-mono text-[10.5px] uppercase tracking-[0.6px] mt-[3px]"
                         style="color: var(--tr-text-faint);">{messages.length} Nachrichten</div>
                </div>
                <button onclick={closeChat}
                        aria-label="Chat schließen"
                        class="w-7 h-7 rounded-[7px] grid place-items-center cursor-pointer transition-colors"
                        style="border: 1px solid var(--tr-line); background: transparent; color: var(--tr-text-dim);">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6L6 18M6 6l12 12"/></svg>
                </button>
            </div>

            <!-- Messages -->
            <div bind:this={chatScrollEl}
                 class="flex-1 overflow-y-auto chat-scroll flex flex-col gap-4 min-h-0"
                 style="padding: 16px 18px;">

                {#if messages.length === 0}
                    <p class="text-xs text-center pt-12" style="color: var(--tr-text-faint);">Noch keine Nachrichten</p>
                {/if}

                {#each messages as msg, i (msg.id)}
                    {@const isOwn = msg.user === currentUser}
                    {@const prevMsg = i > 0 ? messages[i - 1] : null}
                    {@const isFirst = !prevMsg || prevMsg.user !== msg.user}
                    {@const isDiffDay = !prevMsg || formatDate(msg.timestamp) !== formatDate(prevMsg.timestamp)}

                    {#if isDiffDay}
                        <div class="flex items-center gap-2 my-1">
                            <div class="flex-1 h-px" style="background: var(--tr-line);"></div>
                            <span class="font-mono text-[10px] uppercase tracking-[1.2px]"
                                  style="color: var(--tr-text-faint);">{formatDate(msg.timestamp)}</span>
                            <div class="flex-1 h-px" style="background: var(--tr-line);"></div>
                        </div>
                    {/if}

                    <div class="flex gap-2.5 {isOwn ? 'flex-row-reverse' : 'flex-row'} {isFirst ? 'mt-1' : 'mt-[-6px]'}">
                        {#if !isOwn}
                            {#if isFirst}
                                <div class="w-7 h-7 rounded-full grid place-items-center text-[11.5px] font-semibold shrink-0 mb-[2px]"
                                     style="background: var(--tr-surface2); border: 1px solid var(--tr-line); color: var(--tr-text-dim);">
                                    {msg.user.charAt(0).toUpperCase()}
                                </div>
                            {:else}
                                <div class="w-7 shrink-0"></div>
                            {/if}
                        {/if}

                        <div class="flex flex-col {isOwn ? 'items-end' : 'items-start'} max-w-[78%]">
                            {#if isFirst && !isOwn}
                                <div class="font-mono text-[11px] mb-[5px]" style="color: var(--tr-text-dim);">
                                    {msg.user} <span style="color: var(--tr-text-faint);">· {formatTime(msg.timestamp)}</span>
                                </div>
                            {/if}
                            <div class="text-[13px] leading-[1.45] px-[13px] py-2 break-words"
                                 style="
                                     background: {isOwn ? 'var(--tr-red)' : 'var(--tr-surface2)'};
                                     color: {isOwn ? '#fff' : 'var(--tr-text)'};
                                     border-radius: {isOwn ? '12px 12px 3px 12px' : '3px 12px 12px 12px'};
                                     border: {isOwn ? 'none' : '1px solid var(--tr-line)'};
                                 ">{msg.text}</div>
                            {#if isOwn}
                                <div class="font-mono text-[10.5px] mt-[5px]" style="color: var(--tr-text-faint);">
                                    {formatTime(msg.timestamp)}
                                </div>
                            {/if}
                        </div>
                    </div>
                {/each}
            </div>

            <!-- Input -->
            <form onsubmit={(e) => { e.preventDefault(); sendMessage(); }}
                  class="flex gap-2 shrink-0 p-[12px_14px]"
                  style="border-top: 1px solid var(--tr-line);">
                <input type="text"
                       bind:value={chatInput}
                       placeholder="Nachricht…"
                       disabled={isSendingMessage}
                       maxlength={300}
                       onkeydown={(e) => { if (e.key === "Escape") e.currentTarget.blur(); }}
                       class="flex-1 h-9 px-3 rounded-[8px] text-[13px] outline-none transition-colors disabled:opacity-50"
                       style="border: 1px solid var(--tr-line); background: var(--tr-bg); color: var(--tr-text); font-family: 'Inter Variable', sans-serif;" />
                <button type="submit"
                        disabled={isSendingMessage || !chatInput.trim()}
                        aria-label="Senden"
                        class="w-9 h-9 rounded-[8px] grid place-items-center cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed transition-opacity"
                        style="border: none; background: var(--tr-red); color: #fff;">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z"/></svg>
                </button>
            </form>
        </aside>
    </div>
</div>

<style>
    @keyframes mc-pulse {
        0%, 100% { opacity: 1; }
        50%       { opacity: 0.35; }
    }

    .ramp-tile:hover:not(:disabled) {
        transform: scale(1.02) translateY(-1px);
        box-shadow: 0 8px 24px -8px rgba(0, 0, 0, 0.25);
    }
    .ramp-tile:active:not(:disabled) {
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
    .chat-scroll::-webkit-scrollbar { width: 4px; }
    .chat-scroll::-webkit-scrollbar-track { background: transparent; }
    .chat-scroll::-webkit-scrollbar-thumb {
        background: rgba(0, 0, 0, 0.15);
        border-radius: 2px;
    }
    :global(.dark) .chat-scroll::-webkit-scrollbar-thumb {
        background: rgba(255, 255, 255, 0.15);
    }
</style>
