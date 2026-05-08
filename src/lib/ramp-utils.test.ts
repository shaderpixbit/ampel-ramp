import { describe, it, expect } from "vitest";
import {
  isRampLocked,
  cycleRampStatus,
  getStatusLabel,
  formatTime,
  formatDate,
  NEXT_STATUS,
  type RampStatus,
} from "./ramp-utils";

const STATUSES: RampStatus[] = ["free", "pending", "closed"];

// ─── isRampLocked ──────────────────────────────────────────────────────────────

describe("isRampLocked", () => {
  it.each([
    { name: "null lock",      offset: null, expected: false },
    { name: "expired lock",   offset: -1,   expected: false },
    { name: "lock 1ms ahead", offset: 1,    expected: true  },
    { name: "lock 3s ahead",  offset: 3000, expected: true  },
  ])("$name → $expected", ({ offset, expected }) => {
    const locked_until = offset === null ? null : Date.now() + offset;
    expect(isRampLocked({ locked_until })).toBe(expected);
  });
});

// ─── cycleRampStatus / NEXT_STATUS ─────────────────────────────────────────────

describe("cycleRampStatus", () => {
  it.each([
    ["free",    "pending"],
    ["pending", "closed"],
    ["closed",  "free"],
  ] as const)("%s → %s", (input, expected) => {
    expect(cycleRampStatus(input)).toBe(expected);
  });

  // Guards against a regression where two statuses map to the same target,
  // which would collapse the cycle and trap the UI in a 2-state loop.
  it("NEXT_STATUS is a permutation of the statuses", () => {
    expect(new Set(STATUSES.map((s) => NEXT_STATUS[s]))).toEqual(new Set(STATUSES));
  });
});

// ─── getStatusLabel ────────────────────────────────────────────────────────────

describe("getStatusLabel", () => {
  it.each([
    ["free",    "Frei"],
    ["pending", "Wartend"],
    ["closed",  "Belegt"],
  ] as const)("%s → %s", (input, expected) => {
    expect(getStatusLabel(input)).toBe(expected);
  });
});

// ─── formatTime ────────────────────────────────────────────────────────────────

describe("formatTime", () => {
  // Concrete time output is timezone-dependent; assert the shape only.
  it("returns zero-padded HH:MM (5 chars)", () => {
    const result = formatTime("2024-06-15T09:30:00.000Z");
    expect(result).toMatch(/^\d{2}:\d{2}$/);
    expect(result).toHaveLength(5);
  });

  it("pads single-digit hours", () => {
    expect(formatTime("2024-06-15T00:01:00.000Z")).toMatch(/^\d{2}:\d{2}$/);
  });
});

// ─── formatDate ────────────────────────────────────────────────────────────────

describe("formatDate", () => {
  const isoMiddayOffset = (daysAgo: number) => {
    const d = new Date();
    d.setDate(d.getDate() - daysAgo);
    d.setHours(12, 0, 0, 0);
    return d.toISOString();
  };

  it("returns 'Heute' for today", () => {
    expect(formatDate(isoMiddayOffset(0))).toBe("Heute");
  });

  it("returns 'Gestern' for one day ago", () => {
    expect(formatDate(isoMiddayOffset(1))).toBe("Gestern");
  });

  it("falls back to dd.MM.yyyy for older dates", () => {
    expect(formatDate(isoMiddayOffset(7))).toMatch(/^\d{2}\.\d{2}\.\d{4}$/);
  });
});
