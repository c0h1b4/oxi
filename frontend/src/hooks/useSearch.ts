"use client";

import { useQuery } from "@tanstack/react-query";
import { apiGet } from "@/lib/api";
import type { SearchResponse } from "@/types/message";

export function useSearch(query: string, folder?: string) {
  const params = new URLSearchParams();
  if (query) params.set("q", query);
  if (folder) params.set("folder", folder);

  return useQuery({
    queryKey: ["search", query, folder],
    queryFn: () => apiGet<SearchResponse>(`/search?${params.toString()}`),
    enabled: query.length >= 2,
    placeholderData: (prev) => prev,
  });
}
