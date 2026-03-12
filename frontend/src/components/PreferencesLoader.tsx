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
  const theme = useUiStore((s) => s.theme);

  useEffect(() => {
    if (!data) return;
    setDensity(data.density);
    setTheme(data.theme);
    if (data.compose_format) setComposeFormat(data.compose_format);
    localStorage.setItem(THEME_STORAGE_KEY, data.theme);
  }, [data, setDensity, setTheme, setComposeFormat]);

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
