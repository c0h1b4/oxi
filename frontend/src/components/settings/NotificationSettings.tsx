"use client";

import { useCallback } from "react";
import { Loader2 } from "lucide-react";
import { toast } from "sonner";
import {
  useNotificationPreferences,
  useUpdateNotificationPreferences,
} from "@/hooks/useNotificationPreferences";
import { useFolders } from "@/hooks/useFolders";
import { cn } from "@/lib/utils";

// Internal Toggle component for switch-like controls
function Toggle({
  label,
  description,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <div className="flex items-center justify-between rounded-lg border border-border p-4">
      <div>
        <div className="text-sm font-medium">{label}</div>
        <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
      </div>
      <button
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        disabled={disabled}
        className={cn(
          "relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors",
          checked ? "bg-primary" : "bg-muted",
          disabled && "cursor-not-allowed opacity-50",
        )}
      >
        <span
          className={cn(
            "pointer-events-none inline-block size-5 rounded-full bg-background shadow-sm ring-0 transition-transform",
            checked ? "translate-x-5" : "translate-x-0",
          )}
        />
      </button>
    </div>
  );
}

export function NotificationSettings() {
  const { data: prefs, isLoading } = useNotificationPreferences();
  const updatePrefs = useUpdateNotificationPreferences();
  const { data: foldersData } = useFolders();

  const browserPermission =
    typeof window !== "undefined" && "Notification" in window
      ? Notification.permission
      : "denied";

  const handleToggle = useCallback(
    (field: "enabled" | "sound", value: boolean) => {
      updatePrefs.mutate(
        { [field]: value },
        { onError: (e) => toast.error(`Failed to update: ${e.message}`) },
      );
    },
    [updatePrefs],
  );

  const handleFolderToggle = useCallback(
    (folderName: string, add: boolean) => {
      if (!prefs) return;
      const next = add
        ? [...prefs.folders, folderName]
        : prefs.folders.filter((f) => f !== folderName);
      updatePrefs.mutate(
        { folders: next },
        { onError: (e) => toast.error(`Failed to update: ${e.message}`) },
      );
    },
    [prefs, updatePrefs],
  );

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="size-4 animate-spin" />
        Loading notification settings...
      </div>
    );
  }

  if (!prefs) return null;

  const folders = foldersData?.folders ?? [];

  return (
    <div className="max-w-2xl space-y-6">
      <div>
        <h2 className="text-base font-semibold">Notifications</h2>
        <p className="mt-0.5 text-sm text-muted-foreground">
          Configure desktop notification preferences.
        </p>
      </div>

      {browserPermission === "denied" && (
        <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 p-4 text-sm text-amber-700 dark:text-amber-400">
          Browser notifications are blocked. Enable them in your browser settings to receive desktop alerts.
        </div>
      )}

      <div className="space-y-3">
        <Toggle
          label="Desktop notifications"
          description="Show browser notifications when new emails arrive"
          checked={prefs.enabled}
          onChange={(v) => handleToggle("enabled", v)}
        />
        <Toggle
          label="Notification sound"
          description="Play a sound when a notification is shown"
          checked={prefs.sound}
          onChange={(v) => handleToggle("sound", v)}
          disabled={!prefs.enabled}
        />
      </div>

      <div>
        <h3 className="mb-2 text-sm font-medium">Notify for folders</h3>
        <p className="mb-3 text-xs text-muted-foreground">
          Choose which folders trigger notifications.
        </p>
        <div className="space-y-1">
          {folders.map((folder) => {
            const isChecked = prefs.folders.includes(folder.name);
            return (
              <label
                key={folder.name}
                className="flex cursor-pointer items-center gap-2 rounded-md px-3 py-2 text-sm hover:bg-accent"
              >
                <input
                  type="checkbox"
                  checked={isChecked}
                  onChange={(e) => handleFolderToggle(folder.name, e.target.checked)}
                  disabled={!prefs.enabled}
                  className="size-4 rounded border-border"
                />
                {folder.name}
              </label>
            );
          })}
        </div>
      </div>
    </div>
  );
}
