"use client";

import { useMutationState } from "@tanstack/react-query";
import { useAuthStore } from "@/stores/useAuthStore";
import type { SearchResponse } from "@/types/message";

export interface MessageRemoval {
  fromFolder: string;
  uid: number;
  accountId: string | null;
  cancelled: boolean;
  removed: boolean;
}

export function isCurrentRemoval(removal: MessageRemoval) {
  return !removal.cancelled && removal.accountId === useAuthStore.getState().activeAccountId;
}

export function useMessageRemovals({ includeSuccessful = false }: { includeSuccessful?: boolean } = {}) {
  const accountId = useAuthStore(state => state.activeAccountId);
  const removals = useMutationState<MessageRemoval>({
    filters: {
      mutationKey: ["move-message"],
      predicate: mutation => mutation.state.status === "pending"
        || (includeSuccessful && (mutation.state.variables as MessageRemoval).removed),
    },
    select: mutation => mutation.state.variables as MessageRemoval,
  });
  return removals.filter(removal => !removal.cancelled && removal.accountId === accountId);
}

export function hideRemovedSearchResults(data: SearchResponse, removals: readonly Pick<MessageRemoval, "fromFolder" | "uid">[]): SearchResponse {
  // UIDs are only unique within a folder. Filter once even if several actions
  // target the same row, and only subtract results actually present.
  const results = data.results.filter(item => !removals.some(removal =>
    removal.fromFolder === item.folder && removal.uid === item.uid));
  const removed = data.results.length - results.length;
  return removed ? { ...data, results, total_count: Math.max(0, data.total_count - removed) } : data;
}
