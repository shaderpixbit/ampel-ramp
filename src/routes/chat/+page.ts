// Emit a static build/chat.html so the separate chat WebviewWindow can load
// `/chat` directly from the bundled frontend (adapter-static has no SPA server).
export const prerender = true;
