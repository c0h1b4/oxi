"use client";

import { useCallback } from "react";
import { Loader2, Monitor, Moon, Sun } from "lucide-react";
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

  const handleReset = useCallback(() => {
    setDensity("comfortable");
    setTheme("system");
    updatePrefs.mutate(
      { density: "comfortable", theme: "system", language: "en" },
      { onError: (e) => toast.error(`Failed to reset: ${e.message}`) },
    );
  }, [setDensity, setTheme, updatePrefs]);

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
