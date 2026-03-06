"use client";

import { useCallback } from "react";
import { Code2, FileText, Loader2, Monitor, Moon, Search, Sun } from "lucide-react";
import { toast } from "sonner";
import {
  useDisplayPreferences,
  useUpdateDisplayPreferences,
} from "@/hooks/useDisplayPreferences";
import { useUiStore } from "@/stores/useUiStore";
import type { ThemeMode } from "@/stores/useUiStore";
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
    <div className="flex items-center justify-between rounded-lg border border-border p-4">
      <div>
        <div className="text-sm font-medium">{label}</div>
        <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
      </div>
      <div className="flex rounded-lg border border-border bg-muted/50 p-0.5">
        {options.map((opt) => (
          <button
            key={opt.value}
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

  const handleDensityChange = useCallback(
    (density: "compact" | "comfortable") => {
      setDensity(density);
      updatePrefs.mutate(
        { density },
        { onError: (e) => toast.error(`Failed to update: ${e.message}`) },
      );
    },
    [setDensity, updatePrefs],
  );

  const handleThemeChange = useCallback(
    (theme: ThemeMode) => {
      setTheme(theme);
      updatePrefs.mutate(
        { theme },
        { onError: (e) => toast.error(`Failed to update: ${e.message}`) },
      );
    },
    [setTheme, updatePrefs],
  );

  const handleComposeFormatChange = useCallback(
    (compose_format: "html" | "text") => {
      setComposeFormat(compose_format);
      updatePrefs.mutate(
        { compose_format },
        { onError: (e) => toast.error(`Failed to update: ${e.message}`) },
      );
    },
    [setComposeFormat, updatePrefs],
  );

  const handleReset = useCallback(() => {
    setDensity("comfortable");
    setTheme("system");
    setComposeFormat("html");
    updatePrefs.mutate(
      { density: "comfortable", theme: "system", language: "en", compose_format: "html", deep_index: false },
      { onError: (e) => toast.error(`Failed to reset: ${e.message}`) },
    );
  }, [setDensity, setTheme, setComposeFormat, updatePrefs]);

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="size-4 animate-spin" />
        Loading display settings...
      </div>
    );
  }

  if (!prefs) return null;

  return (
    <div className="max-w-2xl space-y-6">
      <div>
        <h2 className="text-base font-semibold">Display</h2>
        <p className="mt-0.5 text-sm text-muted-foreground">
          Customize the appearance and layout of the app.
        </p>
      </div>

      <div className="space-y-3">
        <SegmentedControl
          label="Density"
          description="Controls the spacing of the message list"
          value={prefs.density}
          options={[
            { value: "compact", label: "Compact" },
            { value: "comfortable", label: "Comfortable" },
          ]}
          onChange={handleDensityChange}
        />

        <SegmentedControl
          label="Theme"
          description="Choose light, dark, or match your system"
          value={prefs.theme}
          options={[
            { value: "light", label: "Light", icon: <Sun className="size-3.5" /> },
            { value: "dark", label: "Dark", icon: <Moon className="size-3.5" /> },
            { value: "system", label: "System", icon: <Monitor className="size-3.5" /> },
          ]}
          onChange={handleThemeChange}
        />

        <SegmentedControl
          label="Compose format"
          description="Default format for new emails"
          value={prefs.compose_format}
          options={[
            { value: "html", label: "HTML", icon: <Code2 className="size-3.5" /> },
            { value: "text", label: "Plain text", icon: <FileText className="size-3.5" /> },
          ]}
          onChange={handleComposeFormatChange}
        />

        <SegmentedControl
          label="Search indexing"
          description="Index message bodies for full-text search (uses more bandwidth)"
          value={prefs.deep_index ? "on" : "off"}
          options={[
            { value: "off", label: "Headers only", icon: <Search className="size-3.5" /> },
            { value: "on", label: "Full text", icon: <Search className="size-3.5" /> },
          ]}
          onChange={(v) =>
            updatePrefs.mutate(
              { deep_index: v === "on" },
              { onError: (e) => toast.error(`Failed to update: ${e.message}`) },
            )
          }
        />

        <div className="flex items-center justify-between rounded-lg border border-border p-4">
          <div>
            <div className="text-sm font-medium">Language</div>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Interface language
            </p>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">English</span>
            <span className="rounded-full bg-muted px-2 py-0.5 text-[10px] text-muted-foreground">
              More coming soon
            </span>
          </div>
        </div>
      </div>

      <button
        onClick={handleReset}
        className="text-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
      >
        Reset to defaults
      </button>
    </div>
  );
}
