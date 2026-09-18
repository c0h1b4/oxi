"use client";

import { useEffect } from "react";
import { useDisplayPreferences } from "@/hooks/useDisplayPreferences";
import { normalizeLocale } from "@/lib/i18n";
import { resolveMotionConfig } from "@/lib/motion/config";
import { clearThemeTransitionArtifacts } from "@/lib/motion/theme-spread";
import { useUiStore } from "@/stores/useUiStore";
import type { ThemeMode } from "@/stores/useUiStore";

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
  const setLocale = useUiStore((s) => s.setLocale);
  const theme = useUiStore((s) => s.theme);
  const locale = useUiStore((s) => s.locale);

  useEffect(() => {
    clearThemeTransitionArtifacts();
  }, []);

  useEffect(() => {
    if (!data) return;
    setDensity(data.density);
    setTheme(data.theme);
    if (data.compose_format) setComposeFormat(data.compose_format);
    if (data.language) setLocale(normalizeLocale(data.language));
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
  }, [data, setDensity, setTheme, setComposeFormat, setAnimationModeState, setLocale]);

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

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
