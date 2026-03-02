"use client";
import { createContext, useContext } from "react";
import type { WsStatus } from "@/hooks/useWebSocket";

interface WsContextValue {
  status: WsStatus;
  failCount: number;
}

export const WsContext = createContext<WsContextValue>({
  status: "disconnected",
  failCount: 0,
});

export function useWsStatus() {
  return useContext(WsContext);
}
