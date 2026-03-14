import { afterEach, describe, expect, it, vi } from "vitest";

import { runThemeSpreadTransition } from "../theme-spread";

describe("runThemeSpreadTransition", () => {
  afterEach(() => {
    document.querySelectorAll("[data-theme-transition]").forEach((node) => {
      node.remove();
    });
    document.documentElement.classList.remove("theme-transitioning");
    vi.useRealTimers();
  });

  it("creates spread transition for medium/rich explicit toggles", () => {
    runThemeSpreadTransition({ mode: "medium", trigger: "explicit" });
    const mediumSpread = document.querySelector('[data-theme-transition="spread"]');
    expect(mediumSpread).toBeTruthy();
    expect(mediumSpread?.getAttribute("data-mode")).toBe("medium");

    runThemeSpreadTransition({ mode: "rich", trigger: "explicit" });
    const richSpread = document.querySelector('[data-theme-transition="spread"]');
    expect(richSpread).toBeTruthy();
    expect(richSpread?.getAttribute("data-mode")).toBe("rich");
  });

  it("uses subtle crossfade without spread wipe", () => {
    runThemeSpreadTransition({ mode: "subtle", trigger: "explicit" });

    const fade = document.querySelector('[data-theme-transition="fade"]');
    expect(fade).toBeTruthy();
    expect(document.querySelector('[data-theme-transition="spread"]')).toBeNull();
  });

  it("skips transition effects in off mode", () => {
    runThemeSpreadTransition({ mode: "off", trigger: "explicit" });
    expect(document.querySelector("[data-theme-transition]")).toBeNull();
  });

  it("collapses rapid toggles to latest call", () => {
    vi.useFakeTimers();

    runThemeSpreadTransition({ mode: "medium", trigger: "explicit" });
    runThemeSpreadTransition({ mode: "rich", trigger: "explicit" });

    const spreads = document.querySelectorAll('[data-theme-transition="spread"]');
    expect(spreads).toHaveLength(1);
    expect(spreads[0]?.getAttribute("data-mode")).toBe("rich");

    vi.runAllTimers();
    expect(document.querySelector('[data-theme-transition="spread"]')).toBeNull();
  });

  it("does not run spread on hydration path", () => {
    runThemeSpreadTransition({ mode: "rich", trigger: "hydration" });
    expect(document.querySelector("[data-theme-transition]")).toBeNull();
  });
});
