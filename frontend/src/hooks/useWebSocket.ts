"use client";

import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useAuthStore } from "@/stores/useAuthStore";
import { apiUrl } from "@/lib/api-url";

export type WsStatus = "connected" | "connecting" | "disconnected";

export interface MailEvent {
  type: "NewMessages" | "FlagsChanged" | "FolderUpdated";
  data?: {
    folder: string;
    count?: number;
    latest_sender?: string;
    latest_subject?: string;
  };
}

interface WebSocketMessage {
  accountId: string;
  event: MailEvent;
}

const CONNECT_TIMEOUT_MS = 15_000;

/** Reconnect on resume and refresh mail that may have changed while away. */
export function useWebSocket(onEvent?: (event: MailEvent) => void) {
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<WsStatus>("disconnected");
  const [failCount, setFailCount] = useState(0);
  const onEventRef = useRef(onEvent);
  useEffect(() => {
    onEventRef.current = onEvent;
  });

  useEffect(() => {
    let disposed = false;
    let socket: WebSocket | null = null;
    let backoff = 1000;
    let reconnectTimer: ReturnType<typeof setTimeout> | undefined;
    let connectTimer: ReturnType<typeof setTimeout> | undefined;
    let recoveryTimer: ReturnType<typeof setTimeout> | undefined;
    let flushTimer: ReturnType<typeof setTimeout> | undefined;
    const invalidations = new Map<string, string[]>();

    const refreshMail = () => {
      for (const key of ["messages", "message", "folders", "search"]) {
        void queryClient.invalidateQueries({ queryKey: [key] });
      }
    };

    const scheduleInvalidation = (queryKey: string[]) => {
      invalidations.set(JSON.stringify(queryKey), queryKey);
      clearTimeout(flushTimer);
      flushTimer = setTimeout(() => {
        for (const key of invalidations.values()) {
          void queryClient.invalidateQueries({ queryKey: key });
        }
        invalidations.clear();
      }, 300);
    };

    const disconnect = () => {
      clearTimeout(reconnectTimer);
      clearTimeout(connectTimer);
      if (socket) {
        // Ignore late close/message callbacks from a replaced connection.
        socket.onopen = socket.onclose = socket.onmessage = socket.onerror = null;
        socket.close();
        socket = null;
      }
    };

    const connect = () => {
      if (disposed || !navigator.onLine) return;
      clearTimeout(reconnectTimer);
      setStatus("connecting");

      const wsUrl = new URL(apiUrl("/ws"), window.location.origin);
      wsUrl.protocol = wsUrl.protocol === "https:" ? "wss:" : "ws:";
      const ws = new WebSocket(wsUrl);
      socket = ws;
      const isCurrent = () => !disposed && socket === ws;
      const retry = () => {
        if (!isCurrent()) return;
        disconnect();
        setStatus("disconnected");
        setFailCount((count) => count + 1);
        if (!navigator.onLine) return;
        const delay = backoff;
        backoff = Math.min(delay * 2, 30_000);
        reconnectTimer = setTimeout(connect, delay);
      };
      // A suspended tab can retain a CONNECTING socket indefinitely too.
      connectTimer = setTimeout(retry, CONNECT_TIMEOUT_MS);

      ws.onopen = () => {
        if (!isCurrent()) return;
        clearTimeout(connectTimer);
        setStatus("connected");
        setFailCount(0);
        backoff = 1000;
        refreshMail();
      };

      ws.onmessage = (event) => {
        if (!isCurrent()) return;
        try {
          const message: WebSocketMessage = JSON.parse(event.data);
          if (message.accountId !== useAuthStore.getState().activeAccountId) return;
          const mailEvent = message.event;
          onEventRef.current?.(mailEvent);

          switch (mailEvent.type) {
            case "NewMessages":
            case "FlagsChanged":
              if (mailEvent.data?.folder) {
                scheduleInvalidation(["messages", mailEvent.data.folder]);
              }
              scheduleInvalidation(["folders"]);
              break;
            case "FolderUpdated":
              // Background header sync also uses this event (without a folder).
              scheduleInvalidation(["messages"]);
              scheduleInvalidation(["folders"]);
              break;
          }
        } catch {
          // Ignore malformed messages.
        }
      };
      ws.onclose = retry;
      ws.onerror = () => {
        // onclose handles reconnection; the timeout covers stuck handshakes.
      };
    };

    const recover = () => {
      if (document.visibilityState === "hidden" || !navigator.onLine) return;
      // Visibility, pageshow and focus often arrive together on resume.
      clearTimeout(recoveryTimer);
      recoveryTimer = setTimeout(() => {
        if (disposed) return;
        disconnect();
        backoff = 1000;
        // Do not trust readyState: half-open sockets may still report OPEN.
        connect();
        refreshMail();
      }, 0);
    };
    const offline = () => {
      clearTimeout(recoveryTimer);
      disconnect();
      setStatus("disconnected");
    };

    window.addEventListener("focus", recover);
    window.addEventListener("pageshow", recover);
    window.addEventListener("online", recover);
    window.addEventListener("offline", offline);
    document.addEventListener("visibilitychange", recover);
    document.addEventListener("resume", recover);
    connect();

    return () => {
      disposed = true;
      window.removeEventListener("focus", recover);
      window.removeEventListener("pageshow", recover);
      window.removeEventListener("online", recover);
      window.removeEventListener("offline", offline);
      document.removeEventListener("visibilitychange", recover);
      document.removeEventListener("resume", recover);
      clearTimeout(recoveryTimer);
      clearTimeout(flushTimer);
      disconnect();
    };
  }, [queryClient]);

  return { status, failCount };
}
