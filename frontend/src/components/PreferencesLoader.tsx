"use client";

import { useEffect } from "react";
import { useDisplayPreferences } from "@/hooks/useDisplayPreferences";
import { resolveMotionConfig } from "@/lib/motion/config";
import { useUiStore } from "@/stores/useUiStore";
import type { ThemeMode } from "@/stores/useUiStore";

function clearThemeTransitionArtifacts() {
  document.querySelectorAll("[data-theme-transition]").forEach((node) => {
    node.remove();
  });
  document.documentElement.classList.remove("theme-transitioning");
  document.documentElement.classList.remove("disable-transitions");
  document.documentElement.style.removeProperty("--click-x");
  document.documentElement.style.removeProperty("--click-y");
}

function applyTheme(theme: ThemeMode) {
  const root = document.documentElement;
  if (theme === "dark") {
    root.classList.add("dark");
  } else if (theme === "light") {
    root.classList.remove("dark");
  } else {
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    root.classList.toggle("dark", prefersDark);
  }
}

const THEME_STORAGE_KEY = "oxi-theme";

export function PreferencesLoader() {
  const { data } = useDisplayPreferences();
  const setDensity = useUiStore((s) => s.setDensity);
  const setTheme = useUiStore((s) => s.setTheme);
  const setComposeFormat = useUiStore((s) => s.setComposeFormat);
  const setAnimationModeState = useUiStore((s) => s.setAnimationModeState);
  const theme = useUiStore((s) => s.theme);

  useEffect(() => {
    clearThemeTransitionArtifacts();
  }, []);

  useEffect(() => {
    if (!data) return;
    setDensity(data.density);
    setTheme(data.theme);
    if (data.compose_format) setComposeFormat(data.compose_format);
    const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const motionConfig = resolveMotionConfig(
      data.animation_mode ?? null,
      prefersReducedMotion,
    );
    setAnimationModeState({
      storedMode: motionConfig.storedMode,
      effectiveMode: motionConfig.effectiveMode,
    });
    localStorage.setItem(THEME_STORAGE_KEY, data.theme);
  }, [data, setDensity, setTheme, setComposeFormat, setAnimationModeState]);

  useEffect(() => {
    applyTheme(theme);

    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => applyTheme("system");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  return null;
}
