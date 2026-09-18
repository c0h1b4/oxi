"use client";

import { useCallback } from "react";
import { Code2, FileText, Loader2, Monitor, Moon, Search, Sun } from "lucide-react";
import { toast } from "sonner";
import {
  useDisplayPreferences,
  useUpdateDisplayPreferences,
} from "@/hooks/useDisplayPreferences";
import { useT } from "@/hooks/useI18n";
import {
  DEFAULT_LOCALE,
  SUPPORTED_LOCALES,
  localeName,
  normalizeLocale,
  type Locale,
} from "@/lib/i18n";
import { runThemeSpreadTransition } from "@/lib/motion/theme-spread";
import { useUiStore } from "@/stores/useUiStore";
import type { ThemeMode } from "@/stores/useUiStore";
import type { AnimationMode } from "@/lib/motion/config";
import { cn } from "@/lib/utils";

function SegmentedControl<T extends string>({
  label,
  description,
  value,
  options,
  onChange,
}: {
  label: string;
  description: string;
  value: T;
  options: { value: T; label: string; icon?: React.ReactNode }[];
  onChange: (value: T) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-lg border border-border p-4">
      {/* min-w-0 / shrink-0: translated labels run ~30% longer than English and
          must wrap rather than push the control out of the row. */}
      <div className="min-w-0">
        <div className="text-sm font-medium">{label}</div>
        <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
      </div>
      <div className="flex shrink-0 rounded-lg border border-border bg-muted/50 p-0.5">
        {options.map((opt) => (
          <button
            key={opt.value}
            type="button"
            onClick={() => onChange(opt.value)}
            className={cn(
              "flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
              value === opt.value
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {opt.icon}
            {opt.label}
          </button>
        ))}
      </div>
    </div>
  );
}

export function DisplaySettings() {
  const { data: prefs, isLoading } = useDisplayPreferences();
  const updatePrefs = useUpdateDisplayPreferences();
  const setDensity = useUiStore((s) => s.setDensity);
  const setTheme = useUiStore((s) => s.setTheme);
  const setComposeFormat = useUiStore((s) => s.setComposeFormat);
  const setLocale = useUiStore((s) => s.setLocale);
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const t = useT();

  const handleDensityChange = useCallback(
    (density: "compact" | "comfortable") => {
      setDensity(density);
      updatePrefs.mutate(
        { density },
        {
          onError: (e) =>
            toast.error(t("settings.display.updateFailed", { message: e.message })),
        },
      );
    },
    [setDensity, t, updatePrefs],
  );

  const handleThemeChange = useCallback(
    (theme: ThemeMode) => {
      runThemeSpreadTransition({
        mode: effectiveAnimationMode,
        trigger: "explicit",
        applyTheme: () => setTheme(theme),
        nextTheme: theme,
      });
      updatePrefs.mutate(
        { theme },
        {
          onError: (e) =>
            toast.error(t("settings.display.updateFailed", { message: e.message })),
        },
      );
    },
    [effectiveAnimationMode, setTheme, t, updatePrefs],
  );

  const handleComposeFormatChange = useCallback(
    (compose_format: "html" | "text") => {
      setComposeFormat(compose_format);
      updatePrefs.mutate(
        { compose_format },
        {
          onError: (e) =>
            toast.error(t("settings.display.updateFailed", { message: e.message })),
        },
      );
    },
    [setComposeFormat, t, updatePrefs],
  );

  const handleAnimationModeChange = useCallback(
    (animation_mode: AnimationMode) => {
      updatePrefs.mutate(
        { animation_mode },
        {
          onError: (e) =>
            toast.error(t("settings.display.updateFailed", { message: e.message })),
        },
      );
    },
    [t, updatePrefs],
  );

  const handleLanguageChange = useCallback(
    (language: Locale) => {
      setLocale(language);
      updatePrefs.mutate(
        { language },
        {
          onError: (e) =>
            toast.error(t("settings.display.updateFailed", { message: e.message })),
        },
      );
    },
    [setLocale, t, updatePrefs],
  );

  const handleReset = useCallback(() => {
    setDensity("comfortable");
    setTheme("system");
    setComposeFormat("html");
    setLocale(DEFAULT_LOCALE);
    updatePrefs.mutate(
      {
        density: "comfortable",
        theme: "system",
        language: DEFAULT_LOCALE,
        compose_format: "html",
        animation_mode: null,
        deep_index: false,
      },
      {
        onError: (e) =>
          toast.error(t("settings.display.resetFailed", { message: e.message })),
      },
    );
  }, [setDensity, setTheme, setComposeFormat, setLocale, t, updatePrefs]);

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="size-4 animate-spin" />
        {t("settings.display.loading")}
      </div>
    );
  }

  if (!prefs) return null;

  return (
    <div className="max-w-2xl space-y-6">
      <div>
        <h2 className="text-base font-semibold">{t("settings.display.title")}</h2>
        <p className="mt-0.5 text-sm text-muted-foreground">
          {t("settings.display.subtitle")}
        </p>
      </div>

      <div className="space-y-3">
        <SegmentedControl
          label={t("settings.display.density.label")}
          description={t("settings.display.density.description")}
          value={prefs.density}
          options={[
            { value: "compact", label: t("settings.display.density.compact") },
            { value: "comfortable", label: t("settings.display.density.comfortable") },
          ]}
          onChange={handleDensityChange}
        />

        <SegmentedControl
          label={t("settings.display.theme.label")}
          description={t("settings.display.theme.description")}
          value={prefs.theme}
          options={[
            {
              value: "light",
              label: t("settings.display.theme.light"),
              icon: <Sun className="size-3.5" />,
            },
            {
              value: "dark",
              label: t("settings.display.theme.dark"),
              icon: <Moon className="size-3.5" />,
            },
            {
              value: "system",
              label: t("settings.display.theme.system"),
              icon: <Monitor className="size-3.5" />,
            },
          ]}
          onChange={handleThemeChange}
        />

        <SegmentedControl
          label={t("settings.display.composeFormat.label")}
          description={t("settings.display.composeFormat.description")}
          value={prefs.compose_format}
          options={[
            {
              value: "html",
              label: t("settings.display.composeFormat.html"),
              icon: <Code2 className="size-3.5" />,
            },
            {
              value: "text",
              label: t("settings.display.composeFormat.text"),
              icon: <FileText className="size-3.5" />,
            },
          ]}
          onChange={handleComposeFormatChange}
        />

        <SegmentedControl
          label={t("settings.display.animations.label")}
          description={t("settings.display.animations.description")}
          value={prefs.animation_mode ?? "medium"}
          options={[
            { value: "rich", label: t("settings.display.animations.rich") },
            { value: "medium", label: t("settings.display.animations.medium") },
            { value: "subtle", label: t("settings.display.animations.subtle") },
            { value: "off", label: t("settings.display.animations.off") },
          ]}
          onChange={handleAnimationModeChange}
        />

        <SegmentedControl
          label={t("settings.display.deepIndex.label")}
          description={t("settings.display.deepIndex.description")}
          value={prefs.deep_index ? "on" : "off"}
          options={[
            {
              value: "off",
              label: t("settings.display.deepIndex.off"),
              icon: <Search className="size-3.5" />,
            },
            {
              value: "on",
              label: t("settings.display.deepIndex.on"),
              icon: <Search className="size-3.5" />,
            },
          ]}
          onChange={(v) =>
            updatePrefs.mutate(
              { deep_index: v === "on" },
              {
          onError: (e) =>
            toast.error(t("settings.display.updateFailed", { message: e.message })),
        },
            )
          }
        />

        <SegmentedControl
          label={t("settings.display.language.label")}
          description={t("settings.display.language.description")}
          value={normalizeLocale(prefs.language)}
          options={SUPPORTED_LOCALES.map((locale) => ({
            value: locale,
            label: localeName(locale),
          }))}
          onChange={handleLanguageChange}
        />
      </div>

      <button
        type="button"
        onClick={handleReset}
        className="text-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
      >
        {t("settings.display.reset")}
      </button>
    </div>
  );
}
