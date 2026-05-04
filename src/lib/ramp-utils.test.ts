import { describe, it, expect } from "vitest";
import {
  isRampLocked,
  getStatusColor,
  cycleRampStatus,
  formatDateTime,
  formatTime,
  formatDate,
  NEXT_STATUS,
  type Ramp,
  type RampStatus,
} from "./ramp-utils";

// ─── helpers ───────────────────────────────────────────────────────────────────

function makeRamp(overrides: Partial<Ramp> = {}): Ramp {
  return {
    id: 42,
    name: "Ramp 42",
    status: "free",
    last_updated_by: "TestUser",
    last_updated_at: null,
    locked_until: null,
    ...overrides,
  };
}

function isoAt(offsetMs: number): string {
  return new Date(Date.now() + offsetMs).toISOString();
}

// ─── isRampLocked ──────────────────────────────────────────────────────────────

describe("isRampLocked", () => {
  it("returns false when locked_until is null", () => {
    expect(isRampLocked(makeRamp({ locked_until: null }))).toBe(false);
  });

  it("returns false when lock expired in the past", () => {
    expect(isRampLocked(makeRamp({ locked_until: Date.now() - 1 }))).toBe(false);
  });

  it("returns true when lock is still active", () => {
    expect(isRampLocked(makeRamp({ locked_until: Date.now() + 3000 }))).toBe(true);
  });

  it("returns true for a freshly set 3-second lock", () => {
    expect(isRampLocked(makeRamp({ locked_until: Date.now() + 2999 }))).toBe(true);
  });

  it("works with a Pick — does not require the full Ramp shape", () => {
    expect(isRampLocked({ locked_until: Date.now() + 1000 })).toBe(true);
    expect(isRampLocked({ locked_until: null })).toBe(false);
  });
});

// ─── cycleRampStatus ───────────────────────────────────────────────────────────

describe("cycleRampStatus", () => {
  it("free → pending", () => {
    expect(cycleRampStatus("free")).toBe("pending");
  });

  it("pending → closed", () => {
    expect(cycleRampStatus("pending")).toBe("closed");
  });

  it("closed → free", () => {
    expect(cycleRampStatus("closed")).toBe("free");
  });

  it("cycles back to the start after 3 steps", () => {
    let s: RampStatus = "free";
    s = cycleRampStatus(s);
    s = cycleRampStatus(s);
    s = cycleRampStatus(s);
    expect(s).toBe("free");
  });

  it("NEXT_STATUS covers every known status", () => {
    const statuses: RampStatus[] = ["free", "pending", "closed"];
    for (const s of statuses) {
      expect(statuses).toContain(NEXT_STATUS[s]);
    }
  });
});

// ─── getStatusColor ────────────────────────────────────────────────────────────

describe("getStatusColor", () => {
  it("free returns emerald classes", () => {
    expect(getStatusColor("free")).toContain("emerald");
  });

  it("pending returns amber classes", () => {
    expect(getStatusColor("pending")).toContain("amber");
  });

  it("closed returns rose classes", () => {
    expect(getStatusColor("closed")).toContain("rose");
  });

  it("returns a non-empty string for every known status", () => {
    const statuses: RampStatus[] = ["free", "pending", "closed"];
    for (const s of statuses) {
      expect(getStatusColor(s).length).toBeGreaterThan(0);
    }
  });

  it("free result includes hover variant", () => {
    expect(getStatusColor("free")).toContain("hover:");
  });

  it("closed result includes shadow glow", () => {
    expect(getStatusColor("closed")).toContain("shadow-");
  });
});

// ─── formatDateTime ────────────────────────────────────────────────────────────

describe("formatDateTime", () => {
  it("returns empty strings for null input", () => {
    expect(formatDateTime(null)).toEqual({ date: "", time: "" });
  });

  it("returns an object with date and time keys", () => {
    const result = formatDateTime("2024-06-15T10:30:00.000Z");
    expect(result).toHaveProperty("date");
    expect(result).toHaveProperty("time");
  });

  it("date is non-empty for a valid ISO string", () => {
    expect(formatDateTime("2024-06-15T10:30:00.000Z").date).toBeTruthy();
  });

  it("time is non-empty for a valid ISO string", () => {
    expect(formatDateTime("2024-06-15T10:30:00.000Z").time).toBeTruthy();
  });

  it("time contains HH:MM pattern", () => {
    const { time } = formatDateTime("2024-06-15T10:30:00.000Z");
    expect(time).toMatch(/\d{1,2}:\d{2}/);
  });

  it("date contains digit characters", () => {
    const { date } = formatDateTime("2024-06-15T10:30:00.000Z");
    expect(date).toMatch(/\d/);
  });
});

// ─── formatTime ────────────────────────────────────────────────────────────────

describe("formatTime", () => {
  it("returns HH:MM formatted string", () => {
    const result = formatTime("2024-06-15T14:05:00.000Z");
    expect(result).toMatch(/^\d{2}:\d{2}$/);
  });

  it("always pads to two digits", () => {
    // midnight UTC → single-digit hour in some timezones; format must still be HH:MM
    const result = formatTime("2024-06-15T00:01:00.000Z");
    expect(result).toMatch(/^\d{2}:\d{2}$/);
  });

  it("returns a string of exactly 5 characters", () => {
    expect(formatTime("2024-06-15T09:30:00.000Z")).toHaveLength(5);
  });
});

// ─── formatDate ────────────────────────────────────────────────────────────────

describe("formatDate", () => {
  it('returns "Today" for the current date at midday', () => {
    const today = new Date();
    today.setHours(12, 0, 0, 0);
    expect(formatDate(today.toISOString())).toBe("Today");
  });

  it('returns "Today" for the very start of today', () => {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    expect(formatDate(today.toISOString())).toBe("Today");
  });

  it('returns "Yesterday" for yesterday at midday', () => {
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);
    yesterday.setHours(12, 0, 0, 0);
    expect(formatDate(yesterday.toISOString())).toBe("Yesterday");
  });

  it("does not return Today or Yesterday for two days ago", () => {
    const twoDaysAgo = new Date();
    twoDaysAgo.setDate(twoDaysAgo.getDate() - 2);
    const result = formatDate(twoDaysAgo.toISOString());
    expect(result).not.toBe("Today");
    expect(result).not.toBe("Yesterday");
  });

  it("returns a formatted date string (contains digits) for old dates", () => {
    const old = new Date("2020-03-15T12:00:00.000Z");
    const result = formatDate(old.toISOString());
    expect(result).toMatch(/\d/);
    expect(result).not.toBe("Today");
    expect(result).not.toBe("Yesterday");
  });

  it("uses dot-separated German locale format for old dates", () => {
    const old = new Date("2020-03-15T12:00:00.000Z");
    expect(formatDate(old.toISOString())).toMatch(/\d{2}\.\d{2}\.\d{4}/);
  });
});
