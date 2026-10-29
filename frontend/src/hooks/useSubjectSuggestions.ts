"use client";

import { useQuery } from "@tanstack/react-query";
import { apiGet } from "@/lib/api";
import { useAuthStore } from "@/stores/useAuthStore";
import { useDebouncedValue } from "./useDebouncedValue";

export function useSubjectSuggestions(value: string, enabled = true) {
  const accountId = useAuthStore((s) => s.activeAccountId);
  const query = value.trim();
  const debouncedQuery = useDebouncedValue(query, 200);
  const ready = enabled && !!accountId && query.length >= 2 && query === debouncedQuery;
  const result = useQuery({
    queryKey: ["subject-suggestions", accountId, debouncedQuery],
    queryFn: () => apiGet<{ suggestions: string[] }>(
      `/messages/subject-suggestions?q=${encodeURIComponent(debouncedQuery)}`,
    ),
    enabled: ready,
    staleTime: 30_000,
    retry: false,
  });
  return ready && !result.isError ? result.data?.suggestions ?? [] : [];
}
