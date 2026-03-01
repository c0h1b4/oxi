"use client";

import type { WsStatus } from "@/hooks/useWebSocket";
import { cn } from "@/lib/utils";

interface ConnectionStatusProps {
  status: WsStatus;
  failCount: number;
}

export function ConnectionStatus({ status, failCount }: ConnectionStatusProps) {
  const isReconnecting = status === "connecting" || (status === "disconnected" && failCount < 3);
  const isFailed = status === "disconnected" && failCount >= 3;
  const isConnected = status === "connected";

  const label = isConnected ? "Connected" : isReconnecting ? "Reconnecting..." : "Disconnected";

  return (
    <div className="flex items-center justify-center py-2" title={label}>
      <span
        role="status"
        aria-live="polite"
        aria-label={label}
        className={cn(
          "size-2 rounded-full",
          isConnected && "bg-green-500",
          isReconnecting && "animate-pulse bg-amber-500",
          isFailed && "bg-red-500",
        )}
      />
    </div>
  );
}
