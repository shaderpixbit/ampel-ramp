<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { toggleMode, mode } from "mode-watcher";
  import {
    isRampLocked,
    getStatusColor,
    cycleRampStatus,
    formatDateTime,
    formatTime,
    formatDate,
    type Ramp,
    type RampStatus,
  } from "$lib/ramp-utils";

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
      await tick();
      if (chatScrollEl) chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
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
          if (chatScrollEl) chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
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
      const updatedRamps: Ramp[] = await invoke("update_ramp", { updatedRamp });
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
      const updated: ChatMessage[] = await invoke("send_message", { user: currentUser, text });
      messages = updated;
      await tick();
      if (chatScrollEl) chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
    } catch (e) {
      console.error("Failed to send message:", e);
      chatInput = text;
      showError("Failed to send message. Check your connection to the network drive.");
    } finally {
      isSendingMessage = false;
    }
  }


</script>

<main class="min-h-screen bg-gray-100 dark:bg-[#0f0f0f] text-gray-900 dark:text-white p-2 md:p-6 font-sans overflow-hidden">
  <!-- Error notification -->
  {#if syncError}
    <div class="fixed top-4 left-1/2 -translate-x-1/2 z-50 bg-rose-900/90 border border-rose-500/50 text-rose-100 text-sm font-medium px-5 py-3 rounded-xl shadow-xl backdrop-blur-md">
      {syncError}
    </div>
  {/if}

  <div class="w-full max-w-[100vw] mx-auto flex flex-col h-full">

    <!-- Header -->
    <header class="flex justify-between items-center mb-6 md:mb-12 pb-4 border-b border-black/10 dark:border-white/10 px-2 lg:px-8">
      <!-- Left: title + connection badge -->
      <div>
        <div class="flex items-center gap-3">
          <h1 class="text-3xl md:text-5xl font-extrabold tracking-tight bg-linear-to-r from-gray-900 to-gray-500 dark:from-white dark:to-white/60 bg-clip-text text-transparent">
            Ramp Dashboard
          </h1>
          <div
            class="flex items-center ml-2 gap-1.5 border border-black/10 dark:border-white/10 px-3 py-1 bg-black/5 dark:bg-white/5 rounded-full"
            title={isConnected ? "Connected to Network Drive" : "Disconnected from Network"}
          >
            <div class="w-2.5 h-2.5 rounded-full animate-pulse {isConnected ? 'bg-emerald-500 shadow-[0_0_10px_rgba(16,185,129,0.8)]' : 'bg-red-500 shadow-[0_0_10px_rgba(239,68,68,0.8)]'}"></div>
            <span class="text-[0.6rem] font-bold text-neutral-500 dark:text-neutral-300 uppercase leading-none tracking-wider hidden sm:block">
              {isConnected ? "Drive Sync" : "Lost Sync"}
            </span>
          </div>
        </div>
        <p class="text-neutral-500 dark:text-neutral-400 mt-2 font-medium">Network Drive Flashlight System</p>
      </div>

      <!-- Right: mode toggle + user badge -->
      <div class="flex items-center gap-3">
        <!-- Dark / Light toggle (mode-watcher) -->
        <button
          onclick={toggleMode}
          class="w-9 h-9 flex items-center justify-center rounded-full bg-black/5 dark:bg-white/5 border border-black/10 dark:border-white/10 hover:bg-black/10 dark:hover:bg-white/10 transition-colors shrink-0"
          title={mode.current === "dark" ? "Switch to Light Mode" : "Switch to Dark Mode"}
          aria-label={mode.current === "dark" ? "Switch to Light Mode" : "Switch to Dark Mode"}
        >
          {#if mode.current === "dark"}
            <!-- Sun: click to go light -->
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="w-4 h-4 text-amber-400">
              <path d="M12 2.25a.75.75 0 01.75.75v2.25a.75.75 0 01-1.5 0V3a.75.75 0 01.75-.75zM7.5 12a4.5 4.5 0 119 0 4.5 4.5 0 01-9 0zM18.894 6.166a.75.75 0 00-1.06-1.06l-1.591 1.59a.75.75 0 101.06 1.061l1.591-1.59zM21.75 12a.75.75 0 01-.75.75h-2.25a.75.75 0 010-1.5H21a.75.75 0 01.75.75zM17.834 18.894a.75.75 0 001.06-1.06l-1.59-1.591a.75.75 0 10-1.061 1.06l1.59 1.591zM12 18a.75.75 0 01.75.75V21a.75.75 0 01-1.5 0v-2.25A.75.75 0 0112 18zM7.758 17.303a.75.75 0 00-1.061-1.06l-1.591 1.59a.75.75 0 001.06 1.061l1.591-1.59zM6 12a.75.75 0 01-.75.75H3a.75.75 0 010-1.5h2.25A.75.75 0 016 12zM6.697 7.757a.75.75 0 001.06-1.06l-1.59-1.591a.75.75 0 00-1.061 1.06l1.59 1.591z" />
            </svg>
          {:else}
            <!-- Moon: click to go dark -->
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="w-4 h-4 text-indigo-600">
              <path fill-rule="evenodd" d="M9.528 1.718a.75.75 0 01.162.819A8.97 8.97 0 009 6a9 9 0 009 9 8.97 8.97 0 003.463-.69.75.75 0 01.981.98 10.503 10.503 0 01-9.694 6.46c-5.799 0-10.5-4.701-10.5-10.5 0-4.368 2.667-8.112 6.46-9.694a.75.75 0 01.818.162z" clip-rule="evenodd" />
            </svg>
          {/if}
        </button>

        <!-- User badge -->
        <div class="flex items-center gap-3 bg-black/5 dark:bg-white/5 backdrop-blur-md px-4 py-2 md:px-6 md:py-3 rounded-2xl border border-black/10 dark:border-white/10 shadow-xl">
          <div class="w-10 h-10 rounded-full bg-linear-to-br from-indigo-500 to-purple-600 flex items-center justify-center text-lg font-bold text-white shadow-inner">
            {currentUser.charAt(0).toUpperCase()}
          </div>
          <div class="hidden sm:block">
            <p class="text-sm font-semibold tracking-wide text-gray-900 dark:text-white/90">{currentUser}</p>
            <p class="text-[0.65rem] text-indigo-500 dark:text-indigo-300 mt-0.5 font-medium uppercase tracking-wider">Logged In</p>
          </div>
        </div>
      </div>
    </header>

    <!-- Ramp grid -->
    {#if ramps.length === 0}
      <div class="flex flex-col items-center justify-center h-64 text-neutral-500">
        <div class="w-12 h-12 border-4 border-neutral-300 dark:border-neutral-700 border-t-emerald-500 rounded-full animate-spin mb-4"></div>
        <p class="text-lg font-medium">Connecting...</p>
      </div>
    {:else}
      <div class="grid grid-cols-13 gap-1 sm:gap-2 w-full px-1 lg:px-4 pb-4">
        {#each ramps as ramp (ramp.id)}
          <button
            class="relative flex flex-col items-center justify-center p-1 sm:p-2 lg:p-3 rounded-lg md:rounded-xl border-2
                   transition-all text-center duration-300 transform hover:-translate-y-1 hover:scale-105 active:translate-y-1 active:scale-95 active:shadow-none
                   {getStatusColor(ramp.status)} overflow-hidden group w-full min-h-[100px] md:min-h-[140px] lg:min-h-[180px]
                   {isRampLocked(ramp) ? 'opacity-70' : ''}"
            onclick={() => cycleStatus(ramp)}
            aria-label="Change status of {ramp.name}"
            disabled={!isConnected || isUpdating || isRampLocked(ramp)}
          >
            <div class="absolute inset-x-0 top-0 h-[45%] bg-linear-to-b from-white/30 to-transparent pointer-events-none"></div>
            <div class="absolute inset-0 w-full h-full bg-linear-to-tr from-white/0 via-white/20 to-white/0 opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none transform -skew-x-12 -translate-x-full group-hover:translate-x-full"></div>

            {#if isRampLocked(ramp)}
              <div class="absolute inset-0 rounded-lg md:rounded-xl ring-2 ring-inset ring-white/50 animate-pulse pointer-events-none"></div>
            {/if}

            <span class="text-lg sm:text-xl md:text-2xl lg:text-3xl font-black tracking-tight drop-shadow-sm mb-1">{ramp.id}</span>

            <div class="bg-black/15 rounded-md sm:rounded-lg px-0.5 py-1 w-full mt-auto backdrop-blur-md border border-white/20 shadow-inner overflow-hidden">
              {#if ramp.last_updated_at}
                {@const dt = formatDateTime(ramp.last_updated_at)}
                <div class="flex flex-col justify-center items-center w-full font-bold text-black/70">
                  <span class="truncate max-w-[95%] font-extrabold text-[0.45rem] sm:text-[0.55rem] text-black/90 mb-0.5">{ramp.last_updated_by}</span>
                  <div class="flex flex-col w-full items-center justify-center bg-black/10 py-0.5 rounded leading-none gap-0.5">
                    <span class="text-[0.40rem] sm:text-[0.50rem] md:text-xs opacity-90">{dt.time}</span>
                    <span class="text-[0.40rem] sm:text-[0.50rem] md:text-xs opacity-90">{dt.date}</span>
                  </div>
                </div>
              {:else}
                <div class="flex flex-col justify-center items-center w-full font-bold text-black/50">
                  <span class="truncate max-w-[95%] font-extrabold text-[0.45rem] sm:text-[0.55rem] opacity-90">Unused</span>
                </div>
              {/if}
            </div>
          </button>
        {/each}
      </div>
    {/if}

    <!-- Chat -->
    <div class="px-1 lg:px-4 pb-4 mt-2">
      <div class="border-t border-black/10 dark:border-white/10 pt-4 flex flex-col gap-3 max-w-2xl mx-auto w-full">
        <h2 class="text-xs font-semibold text-neutral-500 uppercase tracking-widest px-1">Team Chat</h2>

        <div
          bind:this={chatScrollEl}
          class="bg-black/5 dark:bg-black/20 border border-black/10 dark:border-white/10 rounded-2xl h-[280px] overflow-y-auto flex flex-col chat-scroll px-3 py-3"
        >
          {#if messages.length === 0}
            <p class="text-neutral-400 dark:text-neutral-600 text-xs text-center m-auto">No messages yet</p>
          {/if}

          {#each messages as msg, i (msg.id)}
            {@const isOwn = msg.user === currentUser}
            {@const prevMsg = i > 0 ? messages[i - 1] : null}
            {@const isFirstFromUser = !prevMsg || prevMsg.user !== msg.user}
            {@const isDifferentDay = !prevMsg || formatDate(msg.timestamp) !== formatDate(prevMsg.timestamp)}

            {#if isDifferentDay}
              <div class="flex items-center gap-2 my-2">
                <div class="flex-1 h-px bg-black/10 dark:bg-white/10"></div>
                <span class="text-[0.6rem] text-neutral-500 uppercase tracking-widest">{formatDate(msg.timestamp)}</span>
                <div class="flex-1 h-px bg-black/10 dark:bg-white/10"></div>
              </div>
            {/if}

            <div class="flex items-end gap-1.5 {isOwn ? 'flex-row-reverse' : 'flex-row'} {isFirstFromUser ? 'mt-3' : 'mt-0.5'}">
              {#if !isOwn}
                {#if isFirstFromUser}
                  <div class="w-7 h-7 rounded-full bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center text-[0.6rem] font-bold text-white shrink-0 mb-0.5 shadow-md">
                    {msg.user.charAt(0).toUpperCase()}
                  </div>
                {:else}
                  <div class="w-7 shrink-0"></div>
                {/if}
              {/if}

              <div class="flex flex-col {isOwn ? 'items-end' : 'items-start'} max-w-[72%]">
                {#if isFirstFromUser && !isOwn}
                  <span class="text-[0.6rem] font-semibold text-indigo-600 dark:text-indigo-400 mb-1 ml-2 truncate max-w-full">{msg.user}</span>
                {/if}
                <div class="px-3 py-1.5 text-xs leading-relaxed break-words {isOwn
                  ? 'bg-indigo-600 text-white rounded-2xl rounded-br-sm shadow-md shadow-indigo-900/40'
                  : 'bg-black/10 text-gray-800 dark:bg-white/10 dark:text-neutral-100 rounded-2xl rounded-bl-sm'}">
                  {msg.text}
                </div>
                <span class="text-[0.55rem] text-neutral-500 dark:text-neutral-600 mt-0.5 mx-2">{formatTime(msg.timestamp)}</span>
              </div>
            </div>
          {/each}
        </div>

        <form onsubmit={(e) => { e.preventDefault(); sendMessage(); }} class="flex items-center gap-2">
          <div class="flex-1 flex items-center bg-black/5 dark:bg-white/5 border border-black/10 dark:border-white/10 rounded-full px-4 py-2 focus-within:border-indigo-500/50 transition-colors">
            <input
              type="text"
              bind:value={chatInput}
              placeholder="Message..."
              disabled={isSendingMessage}
              maxlength={300}
              onkeydown={(e) => { if (e.key === "Escape") e.currentTarget.blur(); }}
              class="flex-1 bg-transparent text-sm text-gray-900 dark:text-white placeholder-neutral-400 dark:placeholder-neutral-600 focus:outline-none disabled:opacity-50"
            />
          </div>
          <button
            type="submit"
            disabled={isSendingMessage || !chatInput.trim()}
            aria-label="Send message"
            class="w-9 h-9 flex items-center justify-center bg-indigo-600 hover:bg-indigo-500 active:bg-indigo-700 disabled:opacity-40 disabled:cursor-not-allowed rounded-full transition-colors shrink-0 shadow-md shadow-indigo-900/40"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="w-4 h-4 translate-x-px -translate-y-px rotate-45">
              <path d="M3.478 2.405a.75.75 0 00-.926.94l2.432 7.905H13.5a.75.75 0 010 1.5H4.984l-2.432 7.905a.75.75 0 00.926.94 60.519 60.519 0 0018.445-8.986.75.75 0 000-1.218A60.517 60.517 0 003.478 2.405z" />
            </svg>
          </button>
        </form>
      </div>
    </div>

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
  .chat-scroll::-webkit-scrollbar { width: 4px; }
  .chat-scroll::-webkit-scrollbar-track { background: transparent; }
  .chat-scroll::-webkit-scrollbar-thumb { background: rgba(0, 0, 0, 0.15); border-radius: 2px; }
  :global(.dark) .chat-scroll::-webkit-scrollbar-thumb { background: rgba(255, 255, 255, 0.15); }
</style>
