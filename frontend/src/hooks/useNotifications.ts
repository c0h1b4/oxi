"use client";

import { useCallback, useEffect, useState } from "react";
import { useUiStore } from "@/stores/useUiStore";
import { useNotificationPreferences } from "./useNotificationPreferences";
import type { MailEvent } from "./useWebSocket";

type NotifPermission = "default" | "granted" | "denied";

export function useNotifications() {
  const [permission, setPermission] = useState<NotifPermission>(() => {
    if (typeof window === "undefined" || !("Notification" in window)) {
      return "denied";
    }
    return Notification.permission as NotifPermission;
  });
  const [bannerDismissed, setBannerDismissed] = useState(() => {
    if (typeof window === "undefined") return false;
    return localStorage.getItem("oxi-notif-banner-dismissed") === "true";
  });

  const { data: prefs } = useNotificationPreferences();
  const setActiveFolder = useUiStore((s) => s.setActiveFolder);

  const showBanner = permission === "default" && !bannerDismissed;

  const requestPermission = useCallback(async () => {
    if (!("Notification" in window)) return;
    const result = await Notification.requestPermission();
    setPermission(result as NotifPermission);
  }, []);

  const dismissBanner = useCallback(() => {
    setBannerDismissed(true);
    localStorage.setItem("oxi-notif-banner-dismissed", "true");
  }, []);

  // Re-check permission on visibility change
  useEffect(() => {
    if (!("Notification" in window)) return;
    const check = () =>
      setPermission(Notification.permission as NotifPermission);
    document.addEventListener("visibilitychange", check);
    return () => document.removeEventListener("visibilitychange", check);
  }, []);

  const handleEvent = useCallback(
    (event: MailEvent) => {
      if (event.type !== "NewMessages") return;
      if (!document.hidden) return;
      if (permission !== "granted") return;
      if (!prefs?.enabled) return;

      const folder = event.data?.folder;
      if (!folder) return;
      if (!prefs.folders.includes(folder)) return;

      const notification = new Notification("New email", {
        body: `New message in ${folder}`,
        tag: `oxi-${folder}-${Date.now()}`,
        icon: "/icon-192.png",
      });

      notification.onclick = () => {
        window.focus();
        setActiveFolder(folder);
        notification.close();
      };
    },
    [permission, prefs, setActiveFolder],
  );

  return {
    permission,
    showBanner,
    requestPermission,
    dismissBanner,
    handleEvent,
  };
}
