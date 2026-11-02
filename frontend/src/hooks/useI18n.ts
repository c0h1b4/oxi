"use client";

import { useMemo, useSyncExternalStore } from "react";

import { createTranslator, DEFAULT_LOCALE, type Locale, type Translator } from "@/lib/i18n";
import { useUiStore } from "@/stores/useUiStore";

/** The active locale, for the date helpers in @/lib/i18n/format. */
export function useLocale(): Locale {
  return useSyncExternalStore(
    useUiStore.subscribe,
    () => useUiStore.getState().locale,
    () => DEFAULT_LOCALE,
  );
}

/** Translation function for the active locale. */
export function useT(): Translator {
  const locale = useLocale();
  return useMemo(() => createTranslator(locale), [locale]);
}
