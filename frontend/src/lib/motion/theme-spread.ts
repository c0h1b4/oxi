import type { AnimationMode } from "@/lib/motion/config";
import { flushSync } from "react-dom";

type ThemeTransitionTrigger = "explicit" | "hydration";
type ThemeTransitionOrigin = { x: number; y: number };
type ThemeMode = "light" | "dark" | "system";
type DocumentWithViewTransition = Document & {
  startViewTransition?: (update: () => void) => { finished: Promise<unknown> };
};

export type ThemeSpreadTransitionInput = {
  mode: AnimationMode;
  trigger: ThemeTransitionTrigger;
  origin?: ThemeTransitionOrigin | null;
  applyTheme?: () => void;
  nextTheme?: ThemeMode;
};

const SUBTLE_DURATION_MS = 140;
const FALLBACK_REVEAL_DURATION_MS = 500;

let activeTransitionId = 0;
let activeCleanup: (() => void) | null = null;

function resolveCurrentBackgroundColor() {
  const rootStyles = window.getComputedStyle(document.documentElement);
  const backgroundVar = rootStyles.getPropertyValue("--background").trim();
  if (backgroundVar) {
    return backgroundVar;
  }

  const bodyBg = window.getComputedStyle(document.body).backgroundColor;
  if (bodyBg) {
    return bodyBg;
  }

  return rootStyles.backgroundColor || "#000";
}

function buildOverlay(kind: "spread" | "fade", mode: AnimationMode) {
  const el = document.createElement("div");
  el.setAttribute("data-theme-transition", kind);
  el.setAttribute("data-mode", mode);
  el.style.position = "fixed";
  el.style.inset = "0";
  el.style.pointerEvents = "none";
  el.style.zIndex = "2147483647";
  el.style.background = resolveCurrentBackgroundColor();
  el.style.opacity = kind === "fade" ? "0.12" : "0.16";
  return el;
}

function resolveTransitionOrigin(origin?: ThemeTransitionOrigin | null): ThemeTransitionOrigin {
  if (
    origin &&
    Number.isFinite(origin.x) &&
    Number.isFinite(origin.y)
  ) {
    return { x: origin.x, y: origin.y };
  }
  return { x: window.innerWidth - 32, y: 32 };
}

function applyThemeChange(applyTheme?: () => void) {
  if (!applyTheme) {
    return;
  }
  flushSync(() => {
    applyTheme();
  });
}

function resolveThemeMode(nextTheme?: ThemeMode): "light" | "dark" {
  const root = document.documentElement;
  if (nextTheme === "dark" || nextTheme === "light") {
    return nextTheme;
  }

  if (typeof window.matchMedia === "function") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  return root.classList.contains("dark") ? "dark" : "light";
}

function applyThemeRootClass(nextTheme?: ThemeMode) {
  const root = document.documentElement;
  const resolvedTheme = resolveThemeMode(nextTheme);
  if (resolvedTheme === "dark") {
    root.classList.add("dark");
    root.classList.remove("light");
    return;
  }

  root.classList.add("light");
  root.classList.remove("dark");
}

function buildCrossfadeOverlay(background: string) {
  const overlay = document.createElement("div");
  overlay.setAttribute("data-theme-transition", "crossfade");
  overlay.style.position = "fixed";
  overlay.style.inset = "0";
  overlay.style.pointerEvents = "none";
  overlay.style.zIndex = "2147483646";
  overlay.style.background = background;
  overlay.style.opacity = "1";
  return overlay;
}

function prefersReducedMotion() {
  if (typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function runSubtle(transitionId: number, applyTheme?: () => void, nextTheme?: ThemeMode) {
  const overlay = buildOverlay("fade", "subtle");
  overlay.style.transition = `opacity ${SUBTLE_DURATION_MS}ms ease-out`;

  document.documentElement.classList.add("theme-transitioning");
  document.body.appendChild(overlay);
  applyThemeChange(applyTheme);
  applyThemeRootClass(nextTheme);

  const frame = window.requestAnimationFrame(() => {
    if (transitionId !== activeTransitionId) {
      return;
    }
    overlay.style.opacity = "0";
  });

  const timeout = window.setTimeout(() => {
    if (transitionId !== activeTransitionId) {
      return;
    }
    overlay.remove();
    document.documentElement.classList.remove("theme-transitioning");
    if (activeCleanup) {
      activeCleanup = null;
    }
  }, SUBTLE_DURATION_MS + 24);

  activeCleanup = () => {
    window.cancelAnimationFrame(frame);
    window.clearTimeout(timeout);
    overlay.remove();
    document.documentElement.classList.remove("theme-transitioning");
  };
}

function runViewTransition(
  transitionId: number,
  origin: ThemeTransitionOrigin | null | undefined,
  applyTheme?: () => void,
  nextTheme?: ThemeMode,
) {
  const documentWithViewTransition = document as DocumentWithViewTransition;
  const startViewTransition = documentWithViewTransition.startViewTransition;
  if (!startViewTransition) {
    return false;
  }

  const root = document.documentElement;
  const transitionOrigin = resolveTransitionOrigin(origin);
  root.style.setProperty("--click-x", `${transitionOrigin.x}px`);
  root.style.setProperty("--click-y", `${transitionOrigin.y}px`);
  root.classList.add("disable-transitions");

  const cleanup = () => {
    root.classList.remove("disable-transitions");
    root.style.removeProperty("--click-x");
    root.style.removeProperty("--click-y");
  };

  try {
    const transition = startViewTransition(() => {
      applyThemeChange(applyTheme);
      applyThemeRootClass(nextTheme);
    });

    transition.finished.finally(() => {
      if (transitionId !== activeTransitionId) {
        return;
      }
      cleanup();
      if (activeCleanup) {
        activeCleanup = null;
      }
    });

    activeCleanup = () => {
      cleanup();
    };

    return true;
  } catch {
    cleanup();
    return false;
  }
}

function runCrossfadeFallback(
  transitionId: number,
  _origin: ThemeTransitionOrigin | null | undefined,
  applyTheme?: () => void,
  nextTheme?: ThemeMode,
) {
  const root = document.documentElement;
  const oldBackground = resolveCurrentBackgroundColor();
  const overlay = buildCrossfadeOverlay(oldBackground);

  root.classList.add("disable-transitions");
  document.body.appendChild(overlay);

  applyThemeChange(applyTheme);
  applyThemeRootClass(nextTheme);

  overlay.style.transition = `opacity ${FALLBACK_REVEAL_DURATION_MS}ms ease-in-out`;

  const frame = window.requestAnimationFrame(() => {
    if (transitionId !== activeTransitionId) {
      return;
    }
    overlay.style.opacity = "0";
  });

  const cleanup = () => {
    window.cancelAnimationFrame(frame);
    overlay.remove();
    root.classList.remove("disable-transitions");
  };

  const timeout = window.setTimeout(() => {
    if (transitionId !== activeTransitionId) {
      return;
    }
    cleanup();
    if (activeCleanup) {
      activeCleanup = null;
    }
  }, FALLBACK_REVEAL_DURATION_MS + 24);

  activeCleanup = () => {
    window.clearTimeout(timeout);
    cleanup();
  };
}

export function clearThemeTransitionArtifacts() {
  document.querySelectorAll("[data-theme-transition]").forEach((node) => {
    node.remove();
  });
  document.documentElement.classList.remove("theme-transitioning");
  document.documentElement.classList.remove("disable-transitions");
  document.documentElement.style.removeProperty("--click-x");
  document.documentElement.style.removeProperty("--click-y");
}

export function runThemeSpreadTransition({
  mode,
  trigger,
  origin,
  applyTheme,
  nextTheme,
}: ThemeSpreadTransitionInput) {
  if (typeof window === "undefined" || typeof document === "undefined") {
    return;
  }

  if (trigger !== "explicit") {
    applyThemeChange(applyTheme);
    applyThemeRootClass(nextTheme);
    return;
  }

  if (activeCleanup) {
    activeCleanup();
    activeCleanup = null;
  }

  activeTransitionId += 1;
  const transitionId = activeTransitionId;

  if (mode === "off") {
    applyThemeChange(applyTheme);
    applyThemeRootClass(nextTheme);
    return;
  }

  if (mode !== "subtle") {
    if (
      !prefersReducedMotion() &&
      runViewTransition(transitionId, origin, applyTheme, nextTheme)
    ) {
      return;
    }

    if (!prefersReducedMotion()) {
      runCrossfadeFallback(transitionId, origin, applyTheme, nextTheme);
      return;
    }

    applyThemeChange(applyTheme);
    applyThemeRootClass(nextTheme);
    return;
  }

  runSubtle(transitionId, applyTheme, nextTheme);
}
