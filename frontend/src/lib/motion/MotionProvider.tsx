"use client";

import { createContext, useContext, useEffect, useMemo, useState } from "react";
import {
  resolveMotionConfig,
  type AnimationMode,
  type MotionConfig,
} from "@/lib/motion/config";
import { useUiStore } from "@/stores/useUiStore";

type StrictMotionModes = Record<AnimationMode, boolean>;

type MotionContextValue = MotionConfig & {
  strictModes: StrictMotionModes;
};

const MotionContext = createContext<MotionContextValue | null>(null);

function getInitialReducedMotionPreference() {
  if (typeof window === "undefined") {
    return false;
  }

  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function buildStrictModes(mode: AnimationMode): StrictMotionModes {
  return {
    rich: mode === "rich",
    medium: mode === "medium",
    subtle: mode === "subtle",
    off: mode === "off",
  };
}

export function MotionProvider({ children }: { children: React.ReactNode }) {
  const storedMode = useUiStore((s) => s.storedAnimationMode);
  const setAnimationModeState = useUiStore((s) => s.setAnimationModeState);
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(
    getInitialReducedMotionPreference,
  );

  useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
    const handleChange = () => setPrefersReducedMotion(mediaQuery.matches);
    mediaQuery.addEventListener("change", handleChange);

    return () => mediaQuery.removeEventListener("change", handleChange);
  }, []);

  const motionConfig = useMemo(
    () => resolveMotionConfig(storedMode, prefersReducedMotion),
    [storedMode, prefersReducedMotion],
  );

  useEffect(() => {
    setAnimationModeState({
      storedMode: motionConfig.storedMode,
      effectiveMode: motionConfig.effectiveMode,
    });
  }, [motionConfig.effectiveMode, motionConfig.storedMode, setAnimationModeState]);

  const value = useMemo<MotionContextValue>(
    () => ({
      ...motionConfig,
      strictModes: buildStrictModes(motionConfig.effectiveMode),
    }),
    [motionConfig],
  );

  return <MotionContext.Provider value={value}>{children}</MotionContext.Provider>;
}

export function useMotionConfig() {
  const context = useContext(MotionContext);

  if (!context) {
    throw new Error("useMotionConfig must be used within MotionProvider");
  }

  return context;
}
