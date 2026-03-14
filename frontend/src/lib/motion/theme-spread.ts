import type { AnimationMode } from "@/lib/motion/config";

type ThemeTransitionTrigger = "explicit" | "hydration";

export type ThemeSpreadTransitionInput = {
  mode: AnimationMode;
  trigger: ThemeTransitionTrigger;
};

const RICH_DURATION_MS = 360;
const MEDIUM_DURATION_MS = 280;
const SUBTLE_DURATION_MS = 140;

let activeTransitionId = 0;
let activeCleanup: (() => void) | null = null;

function buildOverlay(kind: "spread" | "fade", mode: AnimationMode) {
  const el = document.createElement("div");
  el.setAttribute("data-theme-transition", kind);
  el.setAttribute("data-mode", mode);
  el.style.position = "fixed";
  el.style.inset = "0";
  el.style.pointerEvents = "none";
  el.style.zIndex = "2147483647";
  el.style.background = "hsl(var(--background))";
  el.style.opacity = kind === "fade" ? "0.12" : "0.16";
  return el;
}

function runSpread(mode: Extract<AnimationMode, "medium" | "rich">, transitionId: number) {
  const overlay = buildOverlay("spread", mode);
  const duration = mode === "rich" ? RICH_DURATION_MS : MEDIUM_DURATION_MS;

  overlay.style.clipPath = "circle(0% at calc(100% - 2rem) 2rem)";
  overlay.style.transition = `clip-path ${duration}ms cubic-bezier(0.2, 0, 0, 1), opacity ${duration}ms linear`;

  document.documentElement.classList.add("theme-transitioning");
  document.body.appendChild(overlay);

  const frame = window.requestAnimationFrame(() => {
    if (transitionId !== activeTransitionId) {
      return;
    }
    overlay.style.clipPath = "circle(150% at calc(100% - 2rem) 2rem)";
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
  }, duration + 40);

  activeCleanup = () => {
    window.cancelAnimationFrame(frame);
    window.clearTimeout(timeout);
    overlay.remove();
    document.documentElement.classList.remove("theme-transitioning");
  };
}

function runSubtle(transitionId: number) {
  const overlay = buildOverlay("fade", "subtle");
  overlay.style.transition = `opacity ${SUBTLE_DURATION_MS}ms ease-out`;

  document.documentElement.classList.add("theme-transitioning");
  document.body.appendChild(overlay);

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

export function runThemeSpreadTransition({ mode, trigger }: ThemeSpreadTransitionInput) {
  if (typeof window === "undefined" || typeof document === "undefined") {
    return;
  }

  if (trigger !== "explicit") {
    return;
  }

  if (activeCleanup) {
    activeCleanup();
    activeCleanup = null;
  }

  activeTransitionId += 1;
  const transitionId = activeTransitionId;

  if (mode === "off") {
    return;
  }

  if (mode === "subtle") {
    runSubtle(transitionId);
    return;
  }

  runSpread(mode, transitionId);
}
