import { afterEach, describe, expect, it, vi } from "vitest";

import { runThemeSpreadTransition } from "../theme-spread";

describe("runThemeSpreadTransition", () => {
  afterEach(() => {
    document.querySelectorAll("[data-theme-transition]").forEach((node) => {
      node.remove();
    });
    document.documentElement.classList.remove("theme-transitioning");
    document.documentElement.classList.remove("disable-transitions");
    document.documentElement.style.removeProperty("--background");
    document.documentElement.style.removeProperty("--click-x");
    document.documentElement.style.removeProperty("--click-y");
    Object.defineProperty(document, "startViewTransition", {
      value: undefined,
      configurable: true,
      writable: true,
    });
    vi.useRealTimers();
  });

  it("falls back to immediate theme apply when view transitions are unavailable", () => {
    const applyTheme = vi.fn();
    runThemeSpreadTransition({
      mode: "medium",
      trigger: "explicit",
      applyTheme,
      nextTheme: "dark",
    });
    expect(applyTheme).toHaveBeenCalledTimes(1);
    expect(document.documentElement.classList.contains("dark")).toBe(true);

    runThemeSpreadTransition({
      mode: "rich",
      trigger: "explicit",
      applyTheme,
      nextTheme: "light",
    });
    expect(document.querySelector("[data-theme-transition]")).toBeNull();
    expect(applyTheme).toHaveBeenCalledTimes(2);
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("uses subtle crossfade without spread wipe", () => {
    runThemeSpreadTransition({ mode: "subtle", trigger: "explicit" });

    const fade = document.querySelector('[data-theme-transition="fade"]');
    expect(fade).toBeTruthy();
    expect(document.querySelector('[data-theme-transition="spread"]')).toBeNull();
  });

  it("skips transition effects in off mode", () => {
    const applyTheme = vi.fn();
    document.documentElement.classList.add("dark");
    runThemeSpreadTransition({
      mode: "off",
      trigger: "explicit",
      applyTheme,
      nextTheme: "light",
    });
    expect(document.querySelector("[data-theme-transition]")).toBeNull();
    expect(applyTheme).toHaveBeenCalledTimes(1);
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("does not animate on hydration path", () => {
    const applyTheme = vi.fn();
    runThemeSpreadTransition({ mode: "rich", trigger: "hydration", applyTheme });
    expect(document.querySelector("[data-theme-transition]")).toBeNull();
    expect(applyTheme).toHaveBeenCalledTimes(1);
  });

  it("uses view transitions API when available for medium/rich", async () => {
    const applyTheme = vi.fn();
    const finished = Promise.resolve();
    const startViewTransition = vi.fn((update: () => void) => {
      update();
      return { finished } as unknown as ViewTransition;
    });
    Object.defineProperty(document, "startViewTransition", {
      value: startViewTransition as unknown as Document["startViewTransition"],
      configurable: true,
      writable: true,
    });

    runThemeSpreadTransition({
      mode: "rich",
      trigger: "explicit",
      origin: { x: 48, y: 96 },
      applyTheme,
      nextTheme: "dark",
    });

    expect(startViewTransition).toHaveBeenCalledTimes(1);
    expect(applyTheme).toHaveBeenCalledTimes(1);
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.querySelector("[data-theme-transition]")).toBeNull();
    expect(document.documentElement.style.getPropertyValue("--click-x")).toBe("48px");
    expect(document.documentElement.style.getPropertyValue("--click-y")).toBe("96px");
    expect(document.documentElement.classList.contains("disable-transitions")).toBe(true);

    await finished;
    await Promise.resolve();

    expect(document.documentElement.classList.contains("disable-transitions")).toBe(false);
    expect(document.documentElement.style.getPropertyValue("--click-x")).toBe("");
  });
});
