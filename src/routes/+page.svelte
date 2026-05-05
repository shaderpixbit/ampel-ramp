<script lang="ts">
    import { onMount, onDestroy, tick } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { toggleMode } from "mode-watcher";
    import {
        isRampLocked,
        getStatusLabel,
        cycleRampStatus,
        formatDateTime,
        formatTime,
        formatDate,
        type Ramp,
    } from "$lib/ramp-utils";
    import { Button } from "$lib/components/ui/button";
    import {
        MoonIcon,
        SunIcon,
        SendIcon,
        TruckIcon,
        CheckIcon,
        ClockIcon,
        LockIcon,
        LoaderCircleIcon,
    } from "@lucide/svelte";

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

        await fetchRamps();

        try {
            messages = await invoke("get_messages");
            await tick();
            if (chatScrollEl)
                chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
        } catch (e) {
            console.error("Failed to load messages:", e);
        }

        syncTimeout = window.setTimeout(startPolling, 1500);
    });

    onDestroy(() => {
        clearTimeout(syncTimeout);
        clearTimeout(errorTimeout);
    });

    async function startPolling() {
        if (!isUpdating && !isFetching) {
            isFetching = true;
            try {
                const [dbRamps, dbMessages] = await Promise.all([
                    invoke<Ramp[]>("get_ramps"),
                    invoke<ChatMessage[]>("get_messages"),
                ]);

                if (JSON.stringify(ramps) !== JSON.stringify(dbRamps)) {
                    ramps = dbRamps;
                }

                if (JSON.stringify(messages) !== JSON.stringify(dbMessages)) {
                    messages = dbMessages;
                    await tick();
                    if (chatScrollEl)
                        chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
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
            showError("Could not load ramp state from network drive.");
        }
    }

    async function cycleStatus(ramp: Ramp) {
        if (isUpdating || isRampLocked(ramp)) return;

        const newStatus = cycleRampStatus(ramp.status);
        const now = new Date().toISOString();

        const updatedRamp: Ramp = {
            ...ramp,
            status: newStatus,
            last_updated_by: currentUser,
            last_updated_at: now,
        };

        const index = ramps.findIndex((r) => r.id === ramp.id);
        const oldRamp = index !== -1 ? { ...ramps[index] } : null;

        if (index !== -1) {
            ramps[index] = updatedRamp;
        }

        isUpdating = true;
        try {
            const updatedRamps: Ramp[] = await invoke("update_ramp", {
                updatedRamp,
            });
            ramps = updatedRamps;
        } catch (e) {
            console.error("Failed to update ramp:", e);
            if (index !== -1 && oldRamp) {
                ramps[index] = oldRamp;
            }
            showError(`Failed to update Ramp ${ramp.id}. Please try again.`);
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
            const updated: ChatMessage[] = await invoke("send_message", {
                user: currentUser,
                text,
            });
            messages = updated;
            await tick();
            if (chatScrollEl)
                chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
        } catch (e) {
            console.error("Failed to send message:", e);
            chatInput = text;
            showError(
                "Failed to send message. Check your connection to the network drive.",
            );
        } finally {
            isSendingMessage = false;
        }
    }
</script>

<main
    class="h-screen w-screen overflow-hidden bg-zinc-50 dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100 font-sans flex flex-col p-4 gap-4"
>
    {#if syncError}
        <div
            class="fixed top-4 left-1/2 -translate-x-1/2 z-50 bg-rose-600 text-white text-sm font-medium px-4 py-2 rounded-md shadow-lg"
        >
            {syncError}
        </div>
    {/if}

    <!-- Header -->
    <header class="flex items-center justify-between gap-4 shrink-0">
        <div class="flex items-center gap-4">
            <div>
                <h1 class="text-xl font-semibold tracking-tight leading-none">
                    Troiber Ramp Dashboard
                </h1>
                <p
                    class="text-xs text-zinc-500 dark:text-zinc-400 mt-1 font-medium uppercase tracking-wider"
                >
                    Ampel-System
                </p>
            </div>

            <div
                class="flex items-center gap-2 border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 px-2.5 py-1.5 rounded-md"
                title={isConnected
                    ? "Connected to network drive"
                    : "Disconnected from network"}
            >
                <span
                    class="w-2 h-2 rounded-full {isConnected
                        ? 'bg-emerald-500'
                        : 'bg-rose-500'} {isConnected ? '' : 'animate-pulse'}"
                ></span>
                <span
                    class="text-[0.65rem] font-semibold text-zinc-600 dark:text-zinc-300 uppercase tracking-wider"
                >
                    {isConnected ? "Sync" : "Offline"}
                </span>
            </div>
        </div>

        <div class="flex items-center gap-2">
            <Button onclick={toggleMode} variant="outline" size="icon">
                <SunIcon
                    class="h-[1.1rem] w-[1.1rem] scale-100 rotate-0 !transition-all dark:scale-0 dark:-rotate-90"
                />
                <MoonIcon
                    class="absolute h-[1.1rem] w-[1.1rem] scale-0 rotate-90 !transition-all dark:scale-100 dark:rotate-0"
                />
                <span class="sr-only">Toggle theme</span>
            </Button>

            <div
                class="flex items-center gap-2.5 border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 pl-1.5 pr-3 py-1.5 rounded-md"
            >
                <div
                    class="w-7 h-7 rounded-sm bg-zinc-900 dark:bg-zinc-100 flex items-center justify-center text-sm font-semibold text-white dark:text-zinc-900"
                >
                    {currentUser.charAt(0).toUpperCase()}
                </div>
                <div class="leading-tight">
                    <p class="text-xs font-semibold">{currentUser}</p>
                    <p
                        class="text-[0.6rem] text-zinc-500 dark:text-zinc-400 uppercase tracking-wider"
                    >
                        Operator
                    </p>
                </div>
            </div>
        </div>
    </header>

    <!-- Main split: ramp grid + chat sidebar -->
    <div class="flex-1 flex gap-4 min-h-0">
        <!-- Ramp section -->
        <section
            class="flex-1 flex flex-col min-w-0 gap-3 border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 rounded-lg p-4"
        >
            <!-- Section header with counts -->
            <div class="flex items-center justify-between shrink-0">
                <div>
                    <h2 class="text-sm font-semibold uppercase tracking-wider">
                        Rampen
                    </h2>
                    <p class="text-xs text-zinc-500 dark:text-zinc-400 mt-0.5">
                        Klicken um Status zu ändern
                    </p>
                </div>
                <div class="flex items-center gap-2 text-sm font-semibold">
                    <span
                        class="flex items-center gap-2 px-2.5 py-1.5 rounded-md bg-emerald-500/10 text-emerald-700 dark:text-emerald-400 border border-emerald-500/30"
                    >
                        <span
                            class="w-2 h-2 rounded-full bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.6)]"
                        ></span>
                        {counts.free} Frei
                    </span>
                    <span
                        class="flex items-center gap-2 px-2.5 py-1.5 rounded-md bg-amber-500/10 text-amber-700 dark:text-amber-400 border border-amber-500/30"
                    >
                        <span
                            class="w-2 h-2 rounded-full bg-amber-500 shadow-[0_0_8px_rgba(245,158,11,0.6)]"
                        ></span>
                        {counts.pending} Wartend
                    </span>
                    <span
                        class="flex items-center gap-2 px-2.5 py-1.5 rounded-md bg-rose-500/10 text-rose-700 dark:text-rose-400 border border-rose-500/30"
                    >
                        <span
                            class="w-2 h-2 rounded-full bg-rose-500 shadow-[0_0_8px_rgba(244,63,94,0.6)]"
                        ></span>
                        {counts.closed} Belegt
                    </span>
                </div>
            </div>

            <!-- Tile grid -->
            {#if ramps.length === 0}
                <div
                    class="flex-1 flex flex-col items-center justify-center text-zinc-500"
                >
                    <div
                        class="w-8 h-8 border-2 border-zinc-300 dark:border-zinc-700 border-t-emerald-500 rounded-full animate-spin mb-3"
                    ></div>
                    <p class="text-sm font-medium">Verbinde…</p>
                </div>
            {:else}
                <div class="flex-1 grid grid-cols-13 gap-2 min-h-0">
                    {#each ramps as ramp (ramp.id)}
                        {@const locked = isRampLocked(ramp)}
                        {@const occupied = ramp.status !== "free"}
                        {@const docked = ramp.status === "closed"}
                        {@const apronBg =
                            ramp.status === "free"
                                ? "bg-emerald-500"
                                : ramp.status === "pending"
                                  ? "bg-amber-400"
                                  : "bg-rose-500"}
                        {@const doorGradient =
                            ramp.status === "free"
                                ? "bg-gradient-to-b from-emerald-300 to-emerald-600"
                                : ramp.status === "pending"
                                  ? "bg-gradient-to-b from-amber-200 to-amber-500"
                                  : "bg-gradient-to-b from-rose-300 to-rose-600"}
                        {@const glow =
                            ramp.status === "free"
                                ? "shadow-emerald-500/40 hover:shadow-emerald-500/60"
                                : ramp.status === "pending"
                                  ? "shadow-amber-500/40 hover:shadow-amber-500/60"
                                  : "shadow-rose-500/40 hover:shadow-rose-500/60"}
                        {@const iconColor =
                            ramp.status === "free"
                                ? "text-emerald-600 dark:text-emerald-400"
                                : ramp.status === "pending"
                                  ? "text-amber-600 dark:text-amber-400"
                                  : "text-rose-600 dark:text-rose-400"}
                        <button
                            class="relative flex flex-col rounded-xl overflow-hidden border-2 border-white/60 dark:border-zinc-600
                                   shadow-lg {glow}
                                   transition-all duration-200 ease-out text-left
                                   hover:scale-[1.03] hover:-translate-y-0.5 hover:shadow-xl
                                   active:scale-[0.97]
                                   disabled:cursor-not-allowed
                                   {locked ? 'opacity-95' : ''}"
                            onclick={() => cycleStatus(ramp)}
                            aria-label="Ramp {ramp.id}, status: {getStatusLabel(
                                ramp.status,
                            )}. Click to change."
                            disabled={!isConnected || isUpdating || locked}
                        >
                            {#if locked}
                                <div
                                    class="absolute inset-0 z-30 bg-black/20 dark:bg-black/45 flex items-center justify-center pointer-events-none backdrop-blur-[1px]"
                                >
                                    <LoaderCircleIcon
                                        class="w-6 h-6 text-white drop-shadow-md animate-spin"
                                    />
                                </div>
                            {/if}

                            <!-- Building wall: number + status icon -->
                            <div
                                class="flex items-center justify-between px-1.5 pt-1.5 pb-1.5 bg-gradient-to-b from-zinc-200 to-zinc-300 dark:from-zinc-700 dark:to-zinc-800"
                            >
                                <span
                                    class="text-2xl font-extrabold text-zinc-800 dark:text-zinc-100 leading-none tracking-tight"
                                >
                                    {ramp.id}
                                </span>
                                {#if ramp.status === "free"}
                                    <CheckIcon
                                        class="w-4 h-4 {iconColor} drop-shadow-sm shrink-0"
                                    />
                                {:else if ramp.status === "pending"}
                                    <ClockIcon
                                        class="w-4 h-4 {iconColor} drop-shadow-sm shrink-0"
                                    />
                                {:else}
                                    <LockIcon
                                        class="w-4 h-4 {iconColor} drop-shadow-sm shrink-0"
                                    />
                                {/if}
                            </div>

                            <!-- Dock door: thick LED-like bar -->
                            <div
                                class="h-4 {doorGradient} border-y border-zinc-700/30 shadow-inner"
                            ></div>

                            <!-- Apron: full status-colored bay -->
                            <div
                                class="relative flex-1 {apronBg} overflow-hidden shadow-inner"
                            >
                                <!-- Center lane marking -->
                                <div
                                    class="absolute inset-y-2 left-1/2 -translate-x-1/2 w-px border-l-2 border-dashed border-white/40"
                                ></div>

                                {#if occupied}
                                    <div
                                        class="absolute left-1/2 -translate-x-1/2 w-[78%] h-[60%] flex flex-col items-stretch transition-all duration-500 ease-out
                                               {docked
                                            ? 'top-0'
                                            : 'top-[30%] animate-pulse'}"
                                    >
                                        <!-- Trailer -->
                                        <div
                                            class="flex-1 bg-white border-2 border-zinc-800 rounded-md shadow-md flex items-center justify-center"
                                        >
                                            <TruckIcon
                                                class="w-4 h-4 text-zinc-500 rotate-90"
                                            />
                                        </div>
                                        <!-- Cab -->
                                        <div
                                            class="w-[80%] mx-auto bg-zinc-800 dark:bg-zinc-900 border-2 border-zinc-900 rounded-md h-5 mt-0.5 shadow-sm"
                                        ></div>
                                    </div>
                                {/if}
                            </div>

                            <!-- Info strip -->
                            <div
                                class="bg-zinc-100/95 dark:bg-zinc-800/95 backdrop-blur-sm border-t border-zinc-300 dark:border-zinc-700 px-1 py-1 text-center"
                            >
                                {#if ramp.last_updated_at}
                                    {@const dt = formatDateTime(
                                        ramp.last_updated_at,
                                    )}
                                    <p
                                        class="text-[0.55rem] font-bold truncate text-zinc-700 dark:text-zinc-200"
                                    >
                                        {ramp.last_updated_by}
                                    </p>
                                    <p
                                        class="text-[0.5rem] text-zinc-500 dark:text-zinc-400 truncate"
                                    >
                                        {dt.time}
                                    </p>
                                    <p
                                        class="text-[0.5rem] text-zinc-500 dark:text-zinc-400 truncate"
                                    >
                                        {dt.date}
                                    </p>
                                {:else}
                                    <p
                                        class="text-[0.55rem] text-zinc-400 dark:text-zinc-500 italic"
                                    >
                                        —
                                    </p>
                                {/if}
                            </div>
                        </button>
                    {/each}
                </div>
            {/if}
        </section>

        <!-- Chat sidebar -->
        <aside
            class="w-[360px] shrink-0 flex flex-col min-h-0 border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 rounded-lg overflow-hidden"
        >
            <header
                class="flex items-center justify-between px-4 py-3 border-b border-zinc-200 dark:border-zinc-800 shrink-0"
            >
                <div>
                    <h2 class="text-sm font-semibold uppercase tracking-wider">
                        Troiber Team Chat
                    </h2>
                    <p
                        class="text-[0.65rem] text-zinc-500 dark:text-zinc-400 mt-0.5"
                    >
                        {messages.length} Nachrichten
                    </p>
                </div>
            </header>

            <div
                bind:this={chatScrollEl}
                class="flex-1 overflow-y-auto chat-scroll px-3 py-3 min-h-0"
            >
                {#if messages.length === 0}
                    <p
                        class="text-zinc-400 dark:text-zinc-600 text-xs text-center my-auto pt-12"
                    >
                        Noch keine Nachrichten
                    </p>
                {/if}

                {#each messages as msg, i (msg.id)}
                    {@const isOwn = msg.user === currentUser}
                    {@const prevMsg = i > 0 ? messages[i - 1] : null}
                    {@const isFirstFromUser =
                        !prevMsg || prevMsg.user !== msg.user}
                    {@const isDifferentDay =
                        !prevMsg ||
                        formatDate(msg.timestamp) !==
                            formatDate(prevMsg.timestamp)}

                    {#if isDifferentDay}
                        <div class="flex items-center gap-2 my-3">
                            <div
                                class="flex-1 h-px bg-zinc-200 dark:bg-zinc-800"
                            ></div>
                            <span
                                class="text-[0.6rem] font-medium text-zinc-500 uppercase tracking-widest"
                                >{formatDate(msg.timestamp)}</span
                            >
                            <div
                                class="flex-1 h-px bg-zinc-200 dark:bg-zinc-800"
                            ></div>
                        </div>
                    {/if}

                    <div
                        class="flex items-end gap-1.5 {isOwn
                            ? 'flex-row-reverse'
                            : 'flex-row'} {isFirstFromUser
                            ? 'mt-2.5'
                            : 'mt-0.5'}"
                    >
                        {#if !isOwn}
                            {#if isFirstFromUser}
                                <div
                                    class="w-6 h-6 rounded-sm bg-zinc-900 dark:bg-zinc-100 flex items-center justify-center text-[0.6rem] font-semibold text-white dark:text-zinc-900 shrink-0 mb-0.5"
                                >
                                    {msg.user.charAt(0).toUpperCase()}
                                </div>
                            {:else}
                                <div class="w-6 shrink-0"></div>
                            {/if}
                        {/if}

                        <div
                            class="flex flex-col {isOwn
                                ? 'items-end'
                                : 'items-start'} max-w-[75%]"
                        >
                            {#if isFirstFromUser && !isOwn}
                                <span
                                    class="text-[0.6rem] font-semibold text-zinc-600 dark:text-zinc-400 mb-0.5 ml-2 truncate max-w-full"
                                    >{msg.user}</span
                                >
                            {/if}
                            <div
                                class="px-3 py-1.5 text-xs leading-relaxed break-words rounded-md {isOwn
                                    ? 'bg-zinc-900 text-white dark:bg-zinc-100 dark:text-zinc-900'
                                    : 'bg-zinc-100 text-zinc-900 dark:bg-zinc-800 dark:text-zinc-100'}"
                            >
                                {msg.text}
                            </div>
                            <span
                                class="text-[0.55rem] text-zinc-500 dark:text-zinc-500 mt-0.5 mx-1"
                                >{formatTime(msg.timestamp)}</span
                            >
                        </div>
                    </div>
                {/each}
            </div>

            <form
                onsubmit={(e) => {
                    e.preventDefault();
                    sendMessage();
                }}
                class="flex items-center gap-2 p-3 border-t border-zinc-200 dark:border-zinc-800 shrink-0"
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
                    class="flex-1 bg-zinc-100 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 rounded-md px-3 py-1.5 text-xs text-zinc-900 dark:text-zinc-100 placeholder-zinc-400 dark:placeholder-zinc-500 focus:outline-none focus:border-zinc-400 dark:focus:border-zinc-600 transition-colors disabled:opacity-50"
                />
                <button
                    type="submit"
                    disabled={isSendingMessage || !chatInput.trim()}
                    aria-label="Send message"
                    class="w-8 h-8 flex items-center justify-center bg-zinc-900 hover:bg-zinc-800 dark:bg-zinc-100 dark:hover:bg-white text-white dark:text-zinc-900 disabled:opacity-40 disabled:cursor-not-allowed rounded-md transition-colors shrink-0"
                >
                    <SendIcon class="w-3.5 h-3.5" />
                </button>
            </form>
        </aside>
    </div>
</main>

<style>
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
