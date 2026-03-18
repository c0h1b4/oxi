"use client";

import type { WsStatus } from "@/hooks/useWebSocket";
import { cn } from "@/lib/utils";
import { useSyncExternalStore } from "react";

/**
 * Tracks in-flight fetch requests by monkey-patching globalThis.fetch.
 * The LED turns on when any fetch starts and turns off 300ms after the
 * last one settles — so brief idle gaps between sequential requests
 * still look like continuous activity.
 */
let inflight = 0;
let active = false;
const listeners = new Set<() => void>();
let offTimer: ReturnType<typeof setTimeout> | null = null;

function notify() {
  for (const l of listeners) l();
}

function setActive(next: boolean) {
  if (active !== next) {
    active = next;
    notify();
  }
}

if (typeof window !== "undefined") {
  const originalFetch = window.fetch;
  window.fetch = function patchedFetch(...args: Parameters<typeof fetch>) {
    inflight++;
    if (offTimer) { clearTimeout(offTimer); offTimer = null; }
    setActive(true);

    return originalFetch.apply(this, args).finally(() => {
      inflight--;
      if (inflight === 0) {
        offTimer = setTimeout(() => { offTimer = null; setActive(false); }, 300);
      }
    });
  };
}

function subscribe(cb: () => void) {
  listeners.add(cb);
  return () => { listeners.delete(cb); };
}

function getSnapshot() { return active; }
function getServerSnapshot() { return false; }

function useNetworkActivity() {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}

interface ConnectionStatusProps {
  status: WsStatus;
  failCount: number;
}

export function ConnectionStatus({ status, failCount }: ConnectionStatusProps) {
  const isActive = useNetworkActivity();
  const isReconnecting = status === "connecting" || (status === "disconnected" && failCount < 3);
  const isFailed = status === "disconnected" && failCount >= 3;
  const isConnected = status === "connected";

  const label = isConnected
    ? isActive ? "Syncing..." : "Connected"
    : isReconnecting ? "Reconnecting..." : "Disconnected";

  return (
    <div className="flex items-center justify-center py-2" title={label}>
      <span
        role="status"
        aria-live="polite"
        aria-label={label}
        className={cn(
          "size-2 rounded-full",
          isConnected && !isActive && "bg-green-700",
          isConnected && isActive && "animate-hdd-blink",
          isReconnecting && "animate-pulse bg-amber-500",
          isFailed && "bg-red-500",
        )}
      />
    </div>
  );
}
