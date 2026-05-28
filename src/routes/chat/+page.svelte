<script lang="ts">
    import { onMount, onDestroy, tick } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { formatTime, formatDate } from "$lib/ramp-utils";

    const GENERAL = "__general__";

    interface ChatMessage {
        id: string;
        user: string;
        text: string;
        timestamp: string;
        conversation: string | null;
        sender_role: string;
    }

    interface ConversationSummary {
        conversation: string;
        last_text: string | null;
        last_ts: string | null;
        last_sender: string | null;
        unread: number;
        is_legacy: boolean;
    }

    interface ChatOverview {
        version: number;
        conversations: ConversationSummary[] | null;
    }

    interface UserEntry {
        username: string;
        role: string;
    }

    let currentUser = $state("Unknown User");
    let userRole = $state("member_view");
    let isBuero = $derived(
        userRole === "admin" || userRole === "member_buero",
    );
    let isLager = $derived(userRole === "member_lager");
    let hasChat = $derived(isBuero || isLager);

    let conversations = $state<ConversationSummary[]>([]);
    let selectedConv = $state<string | null>(null);
    let threadMessages = $state<ChatMessage[]>([]);
    let chatInput = $state("");
    let isSending = $state(false);
    let scrollEl = $state<HTMLElement>();

    let lagerUsers = $state<string[]>([]);
    let showPicker = $state(false);
    let pendingDelete = $state<ChatMessage | null>(null);

    let lastChatVersion: number | null = null;
    let pollTimeout: number;
    let isSyncing = false;

    let selectedSummary = $derived(
        conversations.find((c) => c.conversation === selectedConv) ?? null,
    );
    // Büro may only type into a real Lager thread (legacy is read-only).
    let canType = $derived(
        !!selectedConv && (isLager || (isBuero && selectedConv !== GENERAL)),
    );

    function convLabel(conv: string, isLegacy: boolean): string {
        if (isLegacy) return "Allgemein (Archiv)";
        return conv;
    }

    function initials(name: string): string {
        return (
            name
                .split(/[.\s_-]/)
                .filter(Boolean)
                .map((p) => p[0]?.toUpperCase() ?? "")
                .slice(0, 2)
                .join("") || "?"
        );
    }

    async function scrollToBottom() {
        await tick();
        if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
    }

    async function loadThread(conv: string, markRead: boolean) {
        try {
            threadMessages = await invoke<ChatMessage[]>("get_chat_thread", {
                username: currentUser,
                role: userRole,
                conversation: conv,
            });
            await scrollToBottom();
            if (markRead) {
                const s = conversations.find((c) => c.conversation === conv);
                if (s && s.unread > 0) {
                    await invoke("mark_chat_read", {
                        username: currentUser,
                        conversation: conv,
                    });
                    lastChatVersion = null; // force a resync so the badge clears
                }
            }
        } catch (e) {
            console.error("Failed to load thread:", e);
        }
    }

    async function selectConv(conv: string) {
        showPicker = false;
        selectedConv = conv;
        await loadThread(conv, true);
    }

    async function startWhisper(lagerUser: string) {
        showPicker = false;
        const conv = lagerUser.toLowerCase();
        selectedConv = conv;
        await loadThread(conv, true);
    }

    async function sendMessage() {
        const text = chatInput.trim();
        if (!text || isSending || !canType || !selectedConv) return;
        chatInput = "";
        isSending = true;
        try {
            await invoke("send_chat_message", {
                sender: currentUser,
                role: userRole,
                conversation: selectedConv,
                text,
            });
            lastChatVersion = null; // force resync
            await loadThread(selectedConv, false);
        } catch (e) {
            console.error("Failed to send message:", e);
            chatInput = text;
        } finally {
            isSending = false;
        }
    }

    async function deleteMessage() {
        const msg = pendingDelete;
        if (!msg || !isBuero) return;
        pendingDelete = null;
        try {
            await invoke("delete_chat_message", {
                role: userRole,
                messageId: msg.id,
            });
            lastChatVersion = null; // force resync
            if (selectedConv) await loadThread(selectedConv, false);
        } catch (e) {
            console.error("Failed to delete message:", e);
        }
    }

    async function sync() {
        if (isSyncing) return;
        isSyncing = true;
        try {
            const ov = await invoke<ChatOverview>("get_chat_overview", {
                username: currentUser,
                role: userRole,
                sinceVersion: lastChatVersion,
            });
            if (ov.conversations !== null) {
                conversations = ov.conversations;
                if (selectedConv) await loadThread(selectedConv, true);
            }
            lastChatVersion = ov.version;
        } catch (e) {
            console.error("Chat sync failed:", e);
        } finally {
            isSyncing = false;
        }
    }

    async function poll() {
        await sync();
        pollTimeout = window.setTimeout(poll, 1500);
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

        if (!hasChat) return;

        await sync();

        if (isLager) {
            // Lager has exactly one thread with the Büro — their own.
            await selectConv(currentUser.toLowerCase());
        } else if (isBuero) {
            try {
                const users = await invoke<UserEntry[]>("get_all_users");
                lagerUsers = users
                    .filter((u) => u.role === "member_lager")
                    .map((u) => u.username);
            } catch (e) {
                console.error("Failed to load Lager users:", e);
            }
        }

        pollTimeout = window.setTimeout(poll, 1500);
    });

    onDestroy(() => clearTimeout(pollTimeout));
</script>

<div
    class="h-screen w-screen overflow-hidden flex"
    style="background: var(--tr-bg); color: var(--tr-text); font-family: 'Inter Variable', sans-serif;"
>
    {#if !hasChat}
        <div
            class="flex-1 flex flex-col items-center justify-center gap-2"
            style="color: var(--tr-text-faint);"
        >
            <p class="text-sm font-medium">Kein Chat-Zugriff</p>
            <p class="text-xs">
                Nur Lager- und Büro-Benutzer können den Chat verwenden.
            </p>
        </div>
    {:else}
        <!-- Conversation sidebar (Büro/Admin only) -->
        {#if isBuero}
            <aside
                class="flex flex-col shrink-0"
                style="width: 240px; background: var(--tr-surface); border-right: 1px solid var(--tr-line);"
            >
                <div
                    class="px-4 py-[14px] shrink-0 flex items-center justify-between"
                    style="border-bottom: 1px solid var(--tr-line);"
                >
                    <div
                        class="text-[13px] font-semibold"
                        style="color: var(--tr-text);"
                    >
                        Posteingang
                    </div>
                    <button
                        onclick={() => (showPicker = !showPicker)}
                        class="h-7 px-2.5 rounded-[7px] flex items-center gap-1 text-[11px] font-medium cursor-pointer"
                        style="border: 1px solid var(--tr-line); background: var(--tr-surface2); color: var(--tr-text-dim);"
                        aria-label="Lager-Mitarbeiter anschreiben"
                    >
                        <svg
                            width="12"
                            height="12"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            ><path d="M12 5v14M5 12h14" /></svg
                        >
                        Lager
                    </button>
                </div>

                {#if showPicker}
                    <div
                        class="max-h-[40%] overflow-y-auto shrink-0"
                        style="border-bottom: 1px solid var(--tr-line); background: var(--tr-surface2);"
                    >
                        {#if lagerUsers.length === 0}
                            <p
                                class="text-[11px] px-4 py-3"
                                style="color: var(--tr-text-faint);"
                            >
                                Keine Lager-Mitarbeiter
                            </p>
                        {/if}
                        {#each lagerUsers as lu (lu)}
                            <button
                                onclick={() => startWhisper(lu)}
                                class="w-full text-left px-4 py-2 text-[12px] cursor-pointer flex items-center gap-2"
                                style="color: var(--tr-text-dim); border-bottom: 1px solid var(--tr-line);"
                            >
                                <span
                                    class="w-5 h-5 rounded-full grid place-items-center text-[9px] font-semibold shrink-0"
                                    style="background: var(--tr-surface); border: 1px solid var(--tr-line);"
                                    >{initials(lu)}</span
                                >
                                {lu}
                            </button>
                        {/each}
                    </div>
                {/if}

                <div class="flex-1 overflow-y-auto">
                    {#if conversations.length === 0}
                        <p
                            class="text-[11px] text-center pt-8"
                            style="color: var(--tr-text-faint);"
                        >
                            Keine Unterhaltungen
                        </p>
                    {/if}
                    {#each conversations as c (c.conversation)}
                        {@const active = c.conversation === selectedConv}
                        <button
                            onclick={() => selectConv(c.conversation)}
                            class="w-full text-left px-4 py-3 cursor-pointer flex flex-col gap-1"
                            style="border-bottom: 1px solid var(--tr-line); background: {active
                                ? 'var(--tr-surface2)'
                                : 'transparent'}; border-left: 3px solid {active
                                ? 'var(--tr-red)'
                                : 'transparent'};"
                        >
                            <div class="flex items-center justify-between gap-2">
                                <span
                                    class="text-[12.5px] font-semibold truncate"
                                    style="color: {c.is_legacy
                                        ? 'var(--tr-text-faint)'
                                        : 'var(--tr-text)'};"
                                    >{convLabel(c.conversation, c.is_legacy)}</span
                                >
                                {#if c.unread > 0}
                                    <span
                                        class="min-w-[17px] h-[17px] px-[5px] rounded-full grid place-items-center text-[9px] font-semibold font-mono shrink-0"
                                        style="background: var(--tr-red); color: #fff;"
                                        >{c.unread}</span
                                    >
                                {/if}
                            </div>
                            {#if c.last_text}
                                <span
                                    class="text-[11px] truncate"
                                    style="color: var(--tr-text-faint);"
                                >
                                    {c.last_sender}: {c.last_text}
                                </span>
                            {/if}
                        </button>
                    {/each}
                </div>
            </aside>
        {/if}

        <!-- Thread pane -->
        <section class="flex-1 flex flex-col min-w-0">
            <!-- Thread header -->
            <div
                class="px-5 py-[14px] shrink-0 flex items-center gap-2.5"
                style="background: var(--tr-surface); border-bottom: 1px solid var(--tr-line);"
            >
                {#if isLager}
                    <div
                        class="w-8 h-8 rounded-full grid place-items-center text-[12px] font-semibold"
                        style="background: var(--tr-red); color: #fff;"
                    >
                        B
                    </div>
                    <div>
                        <div
                            class="text-[14px] font-semibold"
                            style="color: var(--tr-text);"
                        >
                            Büro
                        </div>
                        <div
                            class="font-mono text-[10px] uppercase tracking-[0.6px]"
                            style="color: var(--tr-text-faint);"
                        >
                            Direktnachricht
                        </div>
                    </div>
                {:else if selectedConv}
                    <div
                        class="w-8 h-8 rounded-full grid place-items-center text-[12px] font-semibold"
                        style="background: var(--tr-surface2); border: 1px solid var(--tr-line); color: var(--tr-text-dim);"
                    >
                        {selectedSummary?.is_legacy
                            ? "#"
                            : initials(selectedConv)}
                    </div>
                    <div>
                        <div
                            class="text-[14px] font-semibold"
                            style="color: var(--tr-text);"
                        >
                            {convLabel(
                                selectedConv,
                                selectedSummary?.is_legacy ?? false,
                            )}
                        </div>
                        <div
                            class="font-mono text-[10px] uppercase tracking-[0.6px]"
                            style="color: var(--tr-text-faint);"
                        >
                            {selectedSummary?.is_legacy
                                ? "Nur Lesen"
                                : "Lager-Mitarbeiter"}
                        </div>
                    </div>
                {:else}
                    <div
                        class="text-[13px]"
                        style="color: var(--tr-text-faint);"
                    >
                        Unterhaltung auswählen
                    </div>
                {/if}
            </div>

            <!-- Messages -->
            <div
                bind:this={scrollEl}
                class="flex-1 overflow-y-auto flex flex-col gap-4 min-h-0"
                style="padding: 16px 18px;"
            >
                {#if !selectedConv}
                    <p
                        class="text-xs text-center pt-12"
                        style="color: var(--tr-text-faint);"
                    >
                        {isBuero
                            ? "Wähle links eine Unterhaltung oder schreibe einen Lager-Mitarbeiter an."
                            : ""}
                    </p>
                {:else if threadMessages.length === 0}
                    <p
                        class="text-xs text-center pt-12"
                        style="color: var(--tr-text-faint);"
                    >
                        Noch keine Nachrichten
                    </p>
                {/if}

                {#each threadMessages as msg, i (msg.id)}
                    {@const isOwn = msg.user === currentUser}
                    {@const prev = i > 0 ? threadMessages[i - 1] : null}
                    {@const isFirst = !prev || prev.user !== msg.user}
                    {@const isDiffDay =
                        !prev ||
                        formatDate(msg.timestamp) !==
                            formatDate(prev.timestamp)}

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
                        class="group flex items-center gap-2.5 {isOwn
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
                        {#if isBuero}
                            <button
                                onclick={() => (pendingDelete = msg)}
                                class="w-7 h-7 rounded-full grid place-items-center shrink-0 cursor-pointer opacity-0 group-hover:opacity-100 transition-opacity"
                                style="color: var(--tr-text-faint);"
                                aria-label="Nachricht löschen"
                                title="Nachricht löschen"
                            >
                                <svg
                                    width="14"
                                    height="14"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    ><path
                                        d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m2 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"
                                    /></svg
                                >
                            </button>
                        {/if}
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
                style="background: var(--tr-surface); border-top: 1px solid var(--tr-line);"
            >
                <input
                    type="text"
                    bind:value={chatInput}
                    placeholder={canType
                        ? "Nachricht…"
                        : selectedSummary?.is_legacy
                          ? "Archiv – nur Lesen"
                          : "Unterhaltung auswählen"}
                    disabled={isSending || !canType}
                    maxlength={300}
                    class="flex-1 h-9 px-3 rounded-[8px] text-[13px] outline-none transition-colors disabled:opacity-50"
                    style="border: 1px solid var(--tr-line); background: var(--tr-bg); color: var(--tr-text); font-family: 'Inter Variable', sans-serif;"
                />
                <button
                    type="submit"
                    disabled={isSending || !canType || !chatInput.trim()}
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
        </section>
    {/if}

    {#if pendingDelete}
        <div
            class="fixed inset-0 z-50 flex items-center justify-center"
            style="background: rgba(0,0,0,0.5);"
            onclick={(e) => {
                if (e.target === e.currentTarget) pendingDelete = null;
            }}
            role="presentation"
        >
            <div
                class="rounded-[12px] p-5 flex flex-col gap-4"
                style="width: 340px; background: var(--tr-surface); border: 1px solid var(--tr-line);"
            >
                <div class="flex flex-col gap-1.5">
                    <div
                        class="text-[14px] font-semibold"
                        style="color: var(--tr-text);"
                    >
                        Nachricht löschen?
                    </div>
                    <div
                        class="text-[12px]"
                        style="color: var(--tr-text-dim);"
                    >
                        Diese Nachricht wird endgültig für alle entfernt.
                    </div>
                </div>
                <div
                    class="text-[12.5px] px-3 py-2 rounded-[8px] break-words"
                    style="background: var(--tr-surface2); border: 1px solid var(--tr-line); color: var(--tr-text-dim);"
                >
                    {pendingDelete.text}
                </div>
                <div class="flex gap-2 justify-end">
                    <button
                        onclick={() => (pendingDelete = null)}
                        class="h-8 px-3 rounded-[7px] text-[12px] font-medium cursor-pointer"
                        style="border: 1px solid var(--tr-line); background: var(--tr-surface2); color: var(--tr-text-dim);"
                    >
                        Abbrechen
                    </button>
                    <button
                        onclick={deleteMessage}
                        class="h-8 px-3 rounded-[7px] text-[12px] font-medium cursor-pointer"
                        style="border: none; background: var(--tr-red); color: #fff;"
                    >
                        Löschen
                    </button>
                </div>
            </div>
        </div>
    {/if}
</div>
