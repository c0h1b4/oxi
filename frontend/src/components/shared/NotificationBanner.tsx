"use client";

import { Bell, X } from "lucide-react";

interface NotificationBannerProps {
  onEnable: () => void;
  onDismiss: () => void;
}

export function NotificationBanner({ onEnable, onDismiss }: NotificationBannerProps) {
  return (
    <div className="flex items-center gap-3 border-b border-border bg-accent/50 px-4 py-2">
      <Bell className="size-4 shrink-0 text-primary" />
      <p className="flex-1 text-sm text-foreground">
        Enable desktop notifications to get alerted about new emails.
      </p>
      <button
        onClick={onEnable}
        className="shrink-0 rounded-md bg-primary px-3 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90"
      >
        Enable
      </button>
      <button
        onClick={onDismiss}
        className="shrink-0 rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
        aria-label="Dismiss"
      >
        <X className="size-3.5" />
      </button>
    </div>
  );
}
