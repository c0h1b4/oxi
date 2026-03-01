"use client";

import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

export type WsStatus = "connected" | "connecting" | "disconnected";

export interface MailEvent {
  type: "NewMessages" | "FlagsChanged" | "FolderUpdated";
  data?: { folder: string };
}

/**
 * Connects to the backend WebSocket endpoint for real-time mail events.
 * Automatically reconnects with exponential backoff on disconnect.
 * Invalidates React Query caches when events arrive.
 */
export function useWebSocket(onEvent?: (event: MailEvent) => void) {
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<WsStatus>("disconnected");
  const [failCount, setFailCount] = useState(0);
  const onEventRef = useRef(onEvent);
  useEffect(() => {
    onEventRef.current = onEvent;
  });
  const wsRef = useRef<WebSocket | null>(null);
  const backoffRef = useRef(1000);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mountedRef = useRef(true);
  const connectRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    mountedRef.current = true;

    const connect = () => {
      if (!mountedRef.current) return;

      setStatus("connecting");

      const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
      const ws = new WebSocket(`${protocol}//${window.location.host}/api/ws`);
      wsRef.current = ws;

      ws.onopen = () => {
        if (!mountedRef.current) return;
        setStatus("connected");
        setFailCount(0);
        backoffRef.current = 1000; // Reset backoff on successful connect
      };

      ws.onmessage = (event) => {
        try {
          const mailEvent: MailEvent = JSON.parse(event.data);
          onEventRef.current?.(mailEvent);
          switch (mailEvent.type) {
            case "NewMessages":
              if (mailEvent.data?.folder) {
                queryClient.invalidateQueries({
                  queryKey: ["messages", mailEvent.data.folder],
                });
              }
              queryClient.invalidateQueries({ queryKey: ["folders"] });
              break;
            case "FlagsChanged":
              if (mailEvent.data?.folder) {
                queryClient.invalidateQueries({
                  queryKey: ["messages", mailEvent.data.folder],
                });
              }
              queryClient.invalidateQueries({ queryKey: ["folders"] });
              break;
            case "FolderUpdated":
              queryClient.invalidateQueries({ queryKey: ["folders"] });
              break;
          }
        } catch {
          // Ignore malformed messages
        }
      };

      ws.onclose = () => {
        if (!mountedRef.current) return;
        setStatus("disconnected");
        setFailCount((c) => c + 1);
        wsRef.current = null;

        // Reconnect with exponential backoff
        const delay = backoffRef.current;
        backoffRef.current = Math.min(delay * 2, 30000);
        reconnectTimerRef.current = setTimeout(() => connectRef.current?.(), delay);
      };

      ws.onerror = () => {
        // onclose will fire after onerror, which handles reconnection
      };
    };

    connectRef.current = connect;
    connect();

    return () => {
      mountedRef.current = false;
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
      }
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [queryClient]);

  return { status, failCount };
}
