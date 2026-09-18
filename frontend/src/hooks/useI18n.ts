"use client";

import { useMemo } from "react";

import { createTranslator, type Locale, type Translator } from "@/lib/i18n";
import { useUiStore } from "@/stores/useUiStore";

/** The active locale, for the date helpers in @/lib/i18n/format. */
export function useLocale(): Locale {
  return useUiStore((s) => s.locale);
}

/** Translation function for the active locale. */
export function useT(): Translator {
  const locale = useLocale();
  return useMemo(() => createTranslator(locale), [locale]);
}
