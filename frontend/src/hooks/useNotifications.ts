"use client";

import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { useAuthStore } from "@/stores/useAuthStore";
import { useUiStore } from "@/stores/useUiStore";
import { useNotificationPreferences } from "./useNotificationPreferences";
import type { MailEvent } from "./useWebSocket";

export function useNotifications() {
  const [permission, setPermission] = useState<NotificationPermission>(() => {
    if (typeof window === "undefined" || !("Notification" in window)) {
      return "denied";
    }
    return Notification.permission;
  });
  const [bannerDismissed, setBannerDismissed] = useState(() => {
    if (typeof window === "undefined") return false;
    return localStorage.getItem("oxi-notif-banner-dismissed") === "true";
  });

  const { data: prefs } = useNotificationPreferences();
  const userEmail = useAuthStore((s) => s.email);
  const setActiveFolder = useUiStore((s) => s.setActiveFolder);

  const showBanner = permission === "default" && !bannerDismissed;

  const requestPermission = useCallback(async () => {
    if (!("Notification" in window)) return;
    const result = await Notification.requestPermission();
    setPermission(result);
  }, []);

  const dismissBanner = useCallback(() => {
    setBannerDismissed(true);
    localStorage.setItem("oxi-notif-banner-dismissed", "true");
  }, []);

  // Re-check permission on visibility change
  useEffect(() => {
    if (!("Notification" in window)) return;
    const check = () => setPermission(Notification.permission);
    document.addEventListener("visibilitychange", check);
    return () => document.removeEventListener("visibilitychange", check);
  }, []);

  const handleEvent = useCallback(
    (event: MailEvent) => {
      if (event.type !== "NewMessages") return;
      if (!prefs?.enabled) return;

      const folder = event.data?.folder;
      if (!folder) return;
      if (!prefs.folders.includes(folder)) return;

      const count = event.data?.count ?? 0;
      const sender = event.data?.latest_sender;
      const subject = event.data?.latest_subject;

      // Title: user's own email (the mailbox) or fallback.
      const title = userEmail ?? "New email";

      // Body lines: sender + subject when available.
      const bodyLines: string[] = [];
      if (count > 1) {
        bodyLines.push(`${count} new emails`);
        if (sender) bodyLines.push(`Email from: ${sender}`);
        if (subject) bodyLines.push(`Subject: ${subject}`);
      } else {
        if (sender) bodyLines.push(`Email from: ${sender}`);
        if (subject) bodyLines.push(`Subject: ${subject}`);
        if (bodyLines.length === 0) bodyLines.push("New message received");
      }
      const body = bodyLines.join("\n");

      if (document.hidden || !document.hasFocus()) {
        // Tab is hidden or window is unfocused — show OS notification.
        if (permission === "granted") {
          const notification = new Notification(title, {
            body,
            tag: `oxi-${folder}`,
            icon: "/notification-icon.svg",
          });

          notification.onclick = () => {
            window.focus();
            setActiveFolder(folder);
            notification.close();
          };
        }
      } else {
        // Tab is visible and focused — show in-app toast.
        const toastTitle = sender ? `Email from: ${sender}` : "New email";
        const toastDesc = subject
          ? `Subject: ${subject}`
          : count > 1
            ? `${count} new emails`
            : undefined;
        toast(toastTitle, {
          description: toastDesc,
          action: {
            label: "View",
            onClick: () => setActiveFolder(folder),
          },
        });
      }
    },
    [permission, prefs, userEmail, setActiveFolder],
  );

  return {
    permission,
    showBanner,
    requestPermission,
    dismissBanner,
    handleEvent,
  };
}
