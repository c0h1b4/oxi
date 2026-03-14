import { describe, expect, it } from "vitest";

import {
  DEFAULT_UNSET_NON_REDUCED_MODE,
  getMotionTokens,
  resolveAnimationMode,
  resolveMotionConfig,
  type AnimationMode,
} from "../config";

describe("resolveAnimationMode", () => {
  it("resolves unset + reduced motion to off", () => {
    expect(resolveAnimationMode(null, true)).toBe("off");
    expect(resolveAnimationMode(undefined, true)).toBe("off");
  });

  it("resolves unset + no reduced motion to medium default", () => {
    expect(DEFAULT_UNSET_NON_REDUCED_MODE).toBe("medium");
    expect(resolveAnimationMode(null, false)).toBe("medium");
    expect(resolveAnimationMode(undefined, false)).toBe("medium");
  });

  it("uses explicitly stored mode exactly", () => {
    const modes: AnimationMode[] = ["rich", "medium", "subtle", "off"];
    for (const mode of modes) {
      expect(resolveAnimationMode(mode, true)).toBe(mode);
      expect(resolveAnimationMode(mode, false)).toBe(mode);
    }
  });

  it("treats unknown stored mode as unset", () => {
    expect(resolveAnimationMode("ultra", false)).toBe("medium");
    expect(resolveAnimationMode("ultra", true)).toBe("off");
  });
});

describe("motion token buckets", () => {
  it("returns expected relative duration and distance buckets by mode", () => {
    const rich = getMotionTokens("rich");
    const medium = getMotionTokens("medium");
    const subtle = getMotionTokens("subtle");
    const off = getMotionTokens("off");

    expect(rich.duration.normal).toBeGreaterThan(medium.duration.normal);
    expect(medium.duration.normal).toBeGreaterThan(subtle.duration.normal);
    expect(subtle.duration.normal).toBeGreaterThan(off.duration.normal);

    expect(rich.distance.sm).toBeGreaterThan(medium.distance.sm);
    expect(medium.distance.sm).toBeGreaterThan(subtle.distance.sm);
    expect(subtle.distance.sm).toBeGreaterThan(off.distance.sm);

    expect(off.duration.fast).toBe(0);
    expect(off.duration.normal).toBe(0);
    expect(off.duration.slow).toBe(0);
    expect(off.distance.sm).toBe(0);
  });

  it("returns isolated token objects so callers cannot mutate shared state", () => {
    const firstRich = getMotionTokens("rich");
    const originalNormal = firstRich.duration.normal;

    firstRich.duration.normal = -1;

    const secondRich = getMotionTokens("rich");
    expect(secondRich.duration.normal).toBe(originalNormal);
  });
});

describe("resolveMotionConfig", () => {
  it("falls back to off when resolver throws", () => {
    const config = resolveMotionConfig("medium", false, () => {
      throw new Error("resolver exploded");
    });

    expect(config.effectiveMode).toBe("off");
    expect(config.isOff).toBe(true);
    expect(config.tokens.duration.normal).toBe(0);
    expect(config.tokens.distance.sm).toBe(0);
  });
});
