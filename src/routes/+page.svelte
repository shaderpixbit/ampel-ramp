<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type RampStatus = 'free' | 'pending' | 'closed';

  interface Ramp {
    id: number;
    name: string;
    status: RampStatus;
    last_updated_by: string;
    last_updated_at: string | null;
    locked_until: number | null;
  }

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


  // Chat state
  let messages = $state<ChatMessage[]>([]);
  let chatInput = $state('');
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
        const [dbRamps, dbMessages]: [Ramp[], ChatMessage[]] = await Promise.all([
          invoke("get_ramps"),
          invoke("get_messages"),
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

  function isRampLocked(ramp: Ramp): boolean {
    return !!ramp.locked_until && ramp.locked_until > Date.now();
  }

  async function cycleStatus(ramp: Ramp) {
    if (isUpdating || isRampLocked(ramp)) return;

    const nextStatus: Record<RampStatus, RampStatus> = {
      'free': 'pending',
      'pending': 'closed',
      'closed': 'free'
    };

    const newStatus = nextStatus[ramp.status];
    const now = new Date().toISOString();

    const updatedRamp: Ramp = {
      ...ramp,
      status: newStatus,
      last_updated_by: currentUser,
      last_updated_at: now
    };

    const index = ramps.findIndex(r => r.id === ramp.id);
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

    chatInput = '';
    isSendingMessage = true;
    try {
      const updated: ChatMessage[] = await invoke("send_message", { user: currentUser, text });
      messages = updated;
      await tick();
      if (chatScrollEl) chatScrollEl.scrollTop = chatScrollEl.scrollHeight;
    } catch (e) {
      console.error("Failed to send message:", e);
      chatInput = text;
    } finally {
      isSendingMessage = false;
    }
  }

  function getStatusColor(status: RampStatus) {
    switch (status) {
      case 'free': return 'bg-emerald-500 hover:bg-emerald-400 shadow-[0_0_20px_rgba(16,185,129,0.5)] border-emerald-400 text-emerald-950';
      case 'pending': return 'bg-amber-400 hover:bg-amber-300 shadow-[0_0_20px_rgba(251,191,36,0.5)] border-amber-300 text-amber-950';
      case 'closed': return 'bg-rose-500 hover:bg-rose-400 shadow-[0_0_20px_rgba(225,29,72,0.5)] border-rose-400 text-rose-950';
      default: return 'bg-neutral-800 border-neutral-700 text-neutral-500';
    }
  }

  function formatDateTime(isoString: string | null) {
    if (!isoString) return { date: '', time: '' };
    const d = new Date(isoString);
    return {
      date: d.toLocaleDateString([], { day: '2-digit', month: '2-digit', year: 'numeric' }),
      time: d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    };
  }

  function formatTime(isoString: string) {
    const d = new Date(isoString);
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }
</script>

<main class="min-h-screen bg-[#0f0f0f] text-white p-2 md:p-6 font-sans overflow-hidden">
  <!-- Error notification -->
  {#if syncError}
    <div class="fixed top-4 left-1/2 -translate-x-1/2 z-50 bg-rose-900/90 border border-rose-500/50 text-rose-100 text-sm font-medium px-5 py-3 rounded-xl shadow-xl backdrop-blur-md transition-all">
      {syncError}
    </div>
  {/if}

  <div class="w-full max-w-[100vw] mx-auto flex flex-col h-full">
    <header class="flex justify-between items-center mb-6 md:mb-12 pb-4 border-b border-white/10 px-2 lg:px-8">
      <div>
        <div class="flex items-center gap-3">
          <h1 class="text-3xl md:text-5xl font-extrabold tracking-tight bg-linear-to-r from-white to-white/60 bg-clip-text text-transparent">Ramp Dashboard</h1>
          <!-- Connection Status Indicator -->
          <div class="flex flex-col items-center ml-2 border border-white/10 px-3 py-1 bg-white/5 rounded-full" title={isConnected ? 'Connected to Network Drive' : 'Disconnected from Network'}>
            <div class="flex items-center gap-1.5">
              <div class="w-2.5 h-2.5 rounded-full {isConnected ? 'bg-emerald-500 shadow-[0_0_10px_rgba(16,185,129,0.8)]' : 'bg-red-500 shadow-[0_0_10px_rgba(239,68,68,0.8)]'} animate-pulse"></div>
              <span class="text-[0.6rem] font-bold text-neutral-300 uppercase leading-none tracking-wider hidden sm:block">{isConnected ? 'Drive Sync' : 'Lost Sync'}</span>
            </div>
          </div>
        </div>
        <p class="text-neutral-400 mt-2 font-medium">Network Drive Flashlight System</p>
      </div>

      <div class="flex items-center gap-3 bg-white/5 backdrop-blur-md px-4 py-2 md:px-6 md:py-3 rounded-2xl border border-white/10 shadow-xl">
        <div class="w-10 h-10 rounded-full bg-linear-to-br from-indigo-500 to-purple-600 flex items-center justify-center text-lg font-bold shadow-inner">
          {currentUser.charAt(0).toUpperCase()}
        </div>
        <div class="hidden sm:block">
          <p class="text-sm font-semibold tracking-wide text-white/90">{currentUser}</p>
          <p class="text-[0.65rem] text-indigo-300 mt-0.5 font-medium uppercase tracking-wider">Logged In</p>
        </div>
      </div>
    </header>

    {#if ramps.length === 0}
      <div class="flex flex-col items-center justify-center h-64 text-neutral-500">
        <div class="w-12 h-12 border-4 border-neutral-700 border-t-emerald-500 rounded-full animate-spin mb-4"></div>
        <p class="text-lg font-medium">Connecting...</p>
      </div>
    {:else}
      <!-- Exact 13 column responsive grid spanning the whole width so nothing ever cuts off -->
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
            <!-- Glossy reflection effect overlay -->
            <div class="absolute inset-x-0 top-0 h-[45%] bg-linear-to-b from-white/30 to-transparent pointer-events-none"></div>

            <!-- Glass reflection line -->
            <div class="absolute inset-0 w-full h-full bg-linear-to-tr from-white/0 via-white/20 to-white/0 opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none transform -skew-x-12 -translate-x-full group-hover:translate-x-full"></div>

            <!-- 3-second cooldown pulse ring -->
            {#if isRampLocked(ramp)}
              <div class="absolute inset-0 rounded-lg md:rounded-xl ring-2 ring-inset ring-white/50 animate-pulse pointer-events-none"></div>
            {/if}

            <span class="text-lg sm:text-xl md:text-2xl lg:text-3xl font-black tracking-tight drop-shadow-sm mb-1">{ramp.id}</span>

            <div class="bg-black/15 rounded-md sm:rounded-lg px-0.5 py-1 w-full mt-auto backdrop-blur-md border border-white/20 shadow-inner overflow-hidden">
              {#if ramp.last_updated_at}
                {@const dt = formatDateTime(ramp.last_updated_at)}
                <div class="flex flex-col justify-center items-center w-full text-[0.45rem] sm:text-[0.55rem] font-bold text-black/70">
                  <span class="truncate max-w-[95%] font-extrabold text-[0.45rem] sm:text-[0.55rem] text-black/90 mb-0.5">{ramp.last_updated_by}</span>
                  <div class="flex w-full items-center justify-center bg-black/10 py-0.5 rounded leading-none">
                    <span class="text-[0.40rem] sm:text-[0.50rem] md:text-xs opacity-90 truncate">{dt.time}</span>
                  </div>
                </div>
              {:else}
                <div class="flex flex-col justify-center items-center w-full text-[0.45rem] sm:text-[0.55rem] font-bold text-black/50">
                  <span class="truncate max-w-[95%] font-extrabold text-[0.45rem] sm:text-[0.55rem] opacity-90">Unused</span>
                </div>
              {/if}
            </div>
          </button>
        {/each}
      </div>
    {/if}

    <!-- Chat Section -->
    <div class="px-1 lg:px-4 pb-4 mt-2">
      <div class="border-t border-white/10 pt-4 flex flex-col gap-3">
        <h2 class="text-xs font-semibold text-neutral-500 uppercase tracking-widest px-1">Team Chat</h2>

        <!-- Messages list -->
        <div
          bind:this={chatScrollEl}
          class="bg-black/30 border border-white/10 rounded-xl p-3 h-[150px] overflow-y-auto flex flex-col gap-1.5 chat-scroll"
        >
          {#if messages.length === 0}
            <p class="text-neutral-600 text-xs text-center m-auto">No messages yet</p>
          {/if}
          {#each messages as msg (msg.id)}
            <div class="flex items-baseline gap-2 text-xs min-w-0">
              <span class="font-bold text-indigo-400 shrink-0 max-w-[120px] truncate">{msg.user}</span>
              <span class="text-neutral-200 flex-1 break-words min-w-0">{msg.text}</span>
              <span class="text-neutral-600 text-[0.6rem] shrink-0">{formatTime(msg.timestamp)}</span>
            </div>
          {/each}
        </div>

        <!-- Input row -->
        <form onsubmit={(e) => { e.preventDefault(); sendMessage(); }} class="flex gap-2">
          <input
            type="text"
            bind:value={chatInput}
            placeholder="Send a message to the team..."
            disabled={!isConnected || isSendingMessage}
            maxlength={300}
            class="flex-1 bg-white/5 border border-white/10 rounded-xl px-4 py-2 text-sm text-white placeholder-neutral-600
                   focus:outline-none focus:border-indigo-500/50 focus:bg-white/8 disabled:opacity-50 transition-colors"
          />
          <button
            type="submit"
            disabled={!isConnected || isSendingMessage || !chatInput.trim()}
            class="bg-indigo-600 hover:bg-indigo-500 active:bg-indigo-700 disabled:opacity-40 disabled:cursor-not-allowed
                   text-white text-sm font-semibold px-5 py-2 rounded-xl transition-colors"
          >
            Send
          </button>
        </form>
      </div>
    </div>
  </div>
</main>

<style>
  .hidden-scrollbar {
    -ms-overflow-style: none;
    scrollbar-width: none;
  }
  .hidden-scrollbar::-webkit-scrollbar {
    display: none;
  }

  .chat-scroll {
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.15) transparent;
  }
  .chat-scroll::-webkit-scrollbar {
    width: 4px;
  }
  .chat-scroll::-webkit-scrollbar-track {
    background: transparent;
  }
  .chat-scroll::-webkit-scrollbar-thumb {
    background: rgba(255,255,255,0.15);
    border-radius: 2px;
  }
</style>
