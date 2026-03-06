"use client";

import { useQuery } from "@tanstack/react-query";
import { apiGet } from "@/lib/api";
import type { SearchResponse } from "@/types/message";

export function useSearch(query: string, folder?: string, sort: "date_desc" | "date_asc" = "date_desc") {
  const params = new URLSearchParams();
  if (query) params.set("q", query);
  if (folder) params.set("folder", folder);
  if (sort) params.set("sort", sort);

  return useQuery({
    queryKey: ["search", query, folder, sort],
    queryFn: () => apiGet<SearchResponse>(`/search?${params.toString()}`),
    enabled: query.length >= 2,
    placeholderData: (prev) => prev,
  });
}
