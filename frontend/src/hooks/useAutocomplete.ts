"use client";

import { useQuery } from "@tanstack/react-query";
import { useDebouncedValue } from "./useDebouncedValue";
import { FuzzySearcher, type SearchableItem, type SearchResult } from "@/lib/fuzzySearch";
import { apiGet } from "@/lib/api";

export interface AutocompleteSuggestion {
  email: string;
  name: string;
  source?: "contact" | "known";
}

export interface AutocompleteResult extends SearchResult {
  item: SearchableItem;
}

interface AutocompleteApiResponse {
  suggestions: AutocompleteSuggestion[];
}

const MIN_QUERY_LENGTH = 2;

export function useAutocomplete(
  query: string,
  debounceMs: number = 200
): {
  results: AutocompleteResult[];
  isLoading: boolean;
  error: Error | null;
} {
  const debouncedQuery = useDebouncedValue(query, debounceMs);

  const { data: allContacts, isLoading: isLoadingAll } = useQuery<AutocompleteApiResponse>({
    queryKey: ["contacts-all-for-autocomplete"],
    queryFn: () => apiGet<AutocompleteApiResponse>("/contacts/autocomplete/all"),
    staleTime: 5 * 60 * 1000,
    gcTime: 10 * 60 * 1000,
  });

  const { data: serverResults, isLoading: isLoadingServer, error } = useQuery<AutocompleteApiResponse>({
    queryKey: ["contacts-autocomplete", debouncedQuery],
    queryFn: () =>
      apiGet<AutocompleteApiResponse>(
        `/contacts/autocomplete?q=${encodeURIComponent(debouncedQuery)}&limit=20`
      ),
    enabled: debouncedQuery.length >= MIN_QUERY_LENGTH && !allContacts?.suggestions?.length,
    staleTime: 30000,
  });

  const results = (() => {
    if (debouncedQuery.length < MIN_QUERY_LENGTH) {
      return [];
    }

    if (allContacts?.suggestions?.length) {
      const searchableItems: SearchableItem[] = allContacts.suggestions.map((s) => ({
        email: s.email,
        name: s.name,
        source: s.source ?? "known",
      }));

      const searcher = new FuzzySearcher();
      searcher.setItems(searchableItems);
      return searcher.search(debouncedQuery, 20) as AutocompleteResult[];
    }

    if (serverResults?.suggestions) {
      const searchableItems: SearchableItem[] = serverResults.suggestions.map((s) => ({
        email: s.email,
        name: s.name,
        source: s.source ?? "known",
      }));

      const searcher = new FuzzySearcher();
      searcher.setItems(searchableItems);
      return searcher.search(debouncedQuery, 20) as AutocompleteResult[];
    }

    return [];
  })();

  return {
    results,
    isLoading: isLoadingAll || isLoadingServer,
    error: error,
  };
}

export type { SearchableItem, SearchResult };
