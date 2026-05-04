export type RampStatus = "free" | "pending" | "closed";

export interface Ramp {
  id: number;
  name: string;
  status: RampStatus;
  last_updated_by: string;
  last_updated_at: string | null;
  locked_until: number | null;
}

export const NEXT_STATUS: Record<RampStatus, RampStatus> = {
  free: "pending",
  pending: "closed",
  closed: "free",
};

export function cycleRampStatus(status: RampStatus): RampStatus {
  return NEXT_STATUS[status];
}

export function isRampLocked(ramp: Pick<Ramp, "locked_until">): boolean {
  return !!ramp.locked_until && ramp.locked_until > Date.now();
}

export function getStatusColor(status: RampStatus): string {
  switch (status) {
    case "free":    return "bg-emerald-500 hover:bg-emerald-400 shadow-[0_0_20px_rgba(16,185,129,0.5)] border-emerald-400 text-emerald-950";
    case "pending": return "bg-amber-400 hover:bg-amber-300 shadow-[0_0_20px_rgba(251,191,36,0.5)] border-amber-300 text-amber-950";
    case "closed":  return "bg-rose-500 hover:bg-rose-400 shadow-[0_0_20px_rgba(225,29,72,0.5)] border-rose-400 text-rose-950";
    default:        return "bg-neutral-800 border-neutral-700 text-neutral-500";
  }
}

export function formatDateTime(isoString: string | null): { date: string; time: string } {
  if (!isoString) return { date: "", time: "" };
  const d = new Date(isoString);
  return {
    date: d.toLocaleDateString([], { day: "2-digit", month: "2-digit", year: "numeric" }),
    time: d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
  };
}

export function formatTime(isoString: string): string {
  return new Date(isoString).toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit" });
}

export function formatDate(isoString: string): string {
  const d = new Date(isoString);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  if (d.toDateString() === today.toDateString()) return "Today";
  if (d.toDateString() === yesterday.toDateString()) return "Yesterday";
  return d.toLocaleDateString("de-DE", { day: "2-digit", month: "2-digit", year: "numeric" });
}
