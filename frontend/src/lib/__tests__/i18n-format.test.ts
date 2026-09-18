import { afterEach, describe, expect, it, vi } from "vitest";

import {
  formatListDate,
  formatShortDate,
  humanizeDate,
} from "@/lib/i18n/format";

const NOW = new Date("2026-06-15T12:00:00Z");

function at(offsetMs: number): string {
  return new Date(NOW.getTime() - offsetMs).toISOString();
}

afterEach(() => {
  vi.useRealTimers();
});

function freeze() {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
}

describe("humanizeDate", () => {
  it("uses relative time for the last day", () => {
    freeze();
    expect(humanizeDate("en", at(5 * 60_000))).toBe("5 minutes ago");
    expect(humanizeDate("en", at(60 * 60_000))).toBe("1 hour ago");
  });

  it("pluralises through the locale, not through English strings", () => {
    freeze();
    expect(humanizeDate("es", at(5 * 60_000))).toBe("hace 5 minutos");
    expect(humanizeDate("es", at(60_000))).toBe("hace 1 minuto");
  });

  it("says yesterday in the active language", () => {
    freeze();
    const yesterday = at(26 * 3_600_000);
    expect(humanizeDate("en", yesterday)).toBe("yesterday");
    expect(humanizeDate("es", yesterday)).toBe("ayer");
  });

  it("returns invalid input untouched", () => {
    expect(humanizeDate("en", "not a date")).toBe("not a date");
  });
});

describe("formatListDate", () => {
  it("formats the time of day for today", () => {
    freeze();
    expect(formatListDate("en", at(3_600_000))).toMatch(/AM|PM/);
    expect(formatListDate("es", at(3_600_000))).not.toMatch(/AM|PM/);
  });

  it("returns invalid input untouched", () => {
    expect(formatListDate("en", "nope")).toBe("nope");
  });
});

describe("formatShortDate", () => {
  it("orders the parts the way the locale does", () => {
    expect(formatShortDate("en", "2026-03-09T00:00:00Z")).toContain("2026");
    expect(formatShortDate("es", "2026-03-09T00:00:00Z")).toContain("2026");
    expect(formatShortDate("en", "2026-03-09T00:00:00Z")).not.toEqual(
      formatShortDate("es", "2026-03-09T00:00:00Z"),
    );
  });

  it("returns invalid input untouched", () => {
    expect(formatShortDate("es", "")).toBe("");
  });
});
