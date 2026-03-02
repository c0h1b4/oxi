"use client";

import { Bell, X } from "lucide-react";
import { Button } from "@/components/ui/button";

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
      <Button size="xs" onClick={onEnable}>
        Enable
      </Button>
      <Button variant="ghost" size="icon-xs" onClick={onDismiss} aria-label="Dismiss">
        <X className="size-3.5" />
      </Button>
    </div>
  );
}
