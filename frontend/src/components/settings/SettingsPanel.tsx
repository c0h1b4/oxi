"use client";

import { IdentitySettings } from "./IdentitySettings";
import { NotificationSettings } from "./NotificationSettings";

export function SettingsPanel() {
  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="border-b border-border px-6 py-4">
        <h1 className="text-lg font-semibold">Settings</h1>
      </div>
      <div className="flex-1 space-y-10 overflow-y-auto p-6">
        <IdentitySettings />
        <NotificationSettings />
      </div>
    </div>
  );
}
