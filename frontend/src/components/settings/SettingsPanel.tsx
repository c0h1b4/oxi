"use client";

import { DisplaySettings } from "./DisplaySettings";
import { IdentitySettings } from "./IdentitySettings";
import { NotificationSettings } from "./NotificationSettings";

export function SettingsPanel() {
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <div className="shrink-0 border-b border-border px-4 py-3 sm:px-6 sm:py-4">
        <h1 className="text-lg font-semibold">Settings</h1>
      </div>
      <div className="min-h-0 min-w-0 flex-1 space-y-10 overflow-y-auto p-4 sm:p-6">
        <DisplaySettings />
        <NotificationSettings />
        <IdentitySettings />
      </div>
    </div>
  );
}
