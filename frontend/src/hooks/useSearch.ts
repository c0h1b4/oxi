"use client";

import { useQuery } from "@tanstack/react-query";
import { apiGet } from "@/lib/api";
import { hideRemovedSearchResults, useMessageRemovals } from "./pending-message-removals";
import {
  isValidCommittedSearch,
  normalizeSearchQuery,
} from "@/lib/search-parser";
import type { SearchResponse } from "@/types/message";

export interface SearchFilters {
  from?: string;
  to?: string;
  dateFrom?: string;
  dateTo?: string;
  hasAttachment?: boolean;
  folder?: string;
}

export function useSearch(
  query: string,
  folder?: string,
  sort: "date_desc" | "date_asc" = "date_desc",
  filters?: SearchFilters,
) {
  // Retained successful operations also protect against stale recovery/search
  // responses while overlapping actions are still being reconciled.
  const removals = useMessageRemovals({ includeSuccessful: true });
  const normalizedQuery = normalizeSearchQuery(query);
  const hasValidCommittedQuery = isValidCommittedSearch(normalizedQuery);

  const params = new URLSearchParams();
  if (normalizedQuery) params.set("q", normalizedQuery);
  if (folder) params.set("folder", folder);
  if (sort) params.set("sort", sort);

  if (filters) {
    if (filters.from) params.set("from", filters.from);
    if (filters.to) params.set("to", filters.to);
    if (filters.dateFrom) params.set("date_from", filters.dateFrom);
    if (filters.dateTo) params.set("date_to", filters.dateTo);
    if (filters.hasAttachment !== undefined)
      params.set("has_attachment", String(filters.hasAttachment));
    if (filters.folder) params.set("folder", filters.folder);
  }

  const search = useQuery({
    queryKey: ["search", normalizedQuery, folder, sort, filters],
    queryFn: () => apiGet<SearchResponse>(`/search?${params.toString()}`),
    enabled: hasValidCommittedQuery,
    placeholderData: (prev) => prev,
  });
  // Recovery, polling and query changes can all replace the raw cache while
  // a move/delete is unresolved. Apply the overlay on every read.
  return { ...search, data: search.data ? hideRemovedSearchResults(search.data, removals) : search.data };
}
