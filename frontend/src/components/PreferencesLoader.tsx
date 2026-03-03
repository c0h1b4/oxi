"use client";

import { useEffect } from "react";
import { useDisplayPreferences } from "@/hooks/useDisplayPreferences";
import { useUiStore } from "@/stores/useUiStore";
import type { ThemeMode } from "@/stores/useUiStore";

function applyTheme(theme: ThemeMode) {
  const root = document.documentElement;
  if (theme === "dark") {
    root.classList.add("dark");
  } else if (theme === "light") {
    root.classList.remove("dark");
  } else {
    // system
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    root.classList.toggle("dark", prefersDark);
  }
}

/**
 * Invisible component that loads display preferences from the server
 * and syncs them into the UI store + DOM (theme class).
 */
export function PreferencesLoader() {
  const { data } = useDisplayPreferences();
  const setDensity = useUiStore((s) => s.setDensity);
  const setTheme = useUiStore((s) => s.setTheme);
  const theme = useUiStore((s) => s.theme);

  // Sync server preferences into store when data arrives
  useEffect(() => {
    if (!data) return;
    setDensity(data.density);
    setTheme(data.theme);
  }, [data, setDensity, setTheme]);

  // Apply theme class whenever theme changes
  useEffect(() => {
    applyTheme(theme);

    // Listen for system preference changes when in "system" mode
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => applyTheme("system");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  return null;
}
