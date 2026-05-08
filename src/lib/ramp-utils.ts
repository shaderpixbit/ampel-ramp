export type RampStatus = "free" | "pending" | "closed";

export interface Ramp {
  id: number;
  name: string;
  status: RampStatus;
  last_updated_by: string;
  last_updated_at: string | null;
  locked_until: number | null;
  kennzeichen: string | null;
  notiz: string | null;
  reserviert_fuer: string | null;
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

export function getStatusLabel(status: RampStatus): string {
  switch (status) {
    case "free":    return "Frei";
    case "pending": return "Wartend";
    case "closed":  return "Belegt";
    default:        return "—";
  }
}

export function formatTime(isoString: string): string {
  return new Date(isoString).toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit" });
}

export function formatDate(isoString: string): string {
  const d = new Date(isoString);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  if (d.toDateString() === today.toDateString()) return "Heute";
  if (d.toDateString() === yesterday.toDateString()) return "Gestern";
  return d.toLocaleDateString("de-DE", { day: "2-digit", month: "2-digit", year: "numeric" });
}
