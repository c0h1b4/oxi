"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { HelpCircle, Search, X } from "lucide-react";
import { useUiStore } from "@/stores/useUiStore";
import { useSearch } from "@/hooks/useSearch";
import {
  getFilterLabel,
  isValidCommittedSearch,
  normalizeSearchQuery,
  parseSearchQuery,
  removeFilterFromQuery,
} from "@/lib/search-parser";

const SEARCH_TIPS = [
  { operator: "from:", example: "from:alice@example.com", desc: "Filter by sender" },
  { operator: "to:", example: "to:bob@example.com", desc: "Filter by recipient" },
  { operator: "subject:", example: 'subject:"meeting notes"', desc: "Search subject only" },
  { operator: "in: / folder:", example: "in:Sent", desc: "Filter by folder" },
  { operator: "date:", example: "date:2024-01-15", desc: "Exact date" },
  { operator: "after:", example: "after:2024-01-01", desc: "Messages after date" },
  { operator: "before:", example: "before:2024-06-01", desc: "Messages before date" },
  { operator: "has:attachment", example: "has:attachment", desc: "Has attachments" },
];

export function SearchBar() {
  const searchQuery = useUiStore((s) => s.searchQuery);
  const searchActive = useUiStore((s) => s.searchActive);
  const setSearchQuery = useUiStore((s) => s.setSearchQuery);
  const setSearchActive = useUiStore((s) => s.setSearchActive);
  const clearSearch = useUiStore((s) => s.clearSearch);

  const [inputValue, setInputValue] = useState(searchQuery);
  const [showTips, setShowTips] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const cancelDebounce = useCallback(() => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
      debounceRef.current = null;
    }
  }, []);

  // Fetch results for displaying the count
  const { data } = useSearch(searchQuery);

  // Parse the current query for filter chips
  const parsed = parseSearchQuery(inputValue);

  // Debounce input changes before updating the store
  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const value = e.target.value;
      const normalizedValue = normalizeSearchQuery(value);

      setInputValue(value);
      cancelDebounce();

      if (normalizedValue === "") {
        clearSearch();
        return;
      }

      debounceRef.current = setTimeout(() => {
        if (!isValidCommittedSearch(normalizedValue)) {
          return;
        }

        setSearchQuery(normalizedValue);
        setSearchActive(true);
      }, 300);
    },
    [cancelDebounce, clearSearch, setSearchActive, setSearchQuery],
  );

  // Clear search on Escape
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Escape") {
        setInputValue("");
        cancelDebounce();
        clearSearch();
        setShowTips(false);
        inputRef.current?.blur();
      }
    },
    [cancelDebounce, clearSearch],
  );

  // Clear button handler
  const handleClear = useCallback(() => {
    setInputValue("");
    cancelDebounce();
    clearSearch();
    inputRef.current?.focus();
  }, [cancelDebounce, clearSearch]);

  // Cancel debounce on blur to avoid unexpected state updates
  const handleBlur = useCallback(() => {
    cancelDebounce();
  }, [cancelDebounce]);

  // Remove a filter chip
  const handleRemoveFilter = useCallback(
    (filterRaw: string) => {
      const newQuery = removeFilterFromQuery(inputValue, filterRaw);
      const normalizedQuery = normalizeSearchQuery(newQuery);

      setInputValue(newQuery);
      cancelDebounce();

      if (normalizedQuery === "") {
        clearSearch();
        return;
      }

      if (isValidCommittedSearch(normalizedQuery)) {
        setSearchQuery(normalizedQuery);
        setSearchActive(true);
      }
    },
    [cancelDebounce, clearSearch, inputValue, setSearchActive, setSearchQuery],
  );

  // Insert a tip operator into the input
  const handleTipClick = useCallback(
    (operator: string) => {
      const newValue = inputValue ? `${inputValue} ${operator}` : operator;
      setInputValue(newValue);
      setShowTips(false);
      inputRef.current?.focus();
    },
    [inputValue],
  );

  // Global Cmd/Ctrl+K shortcut to focus search
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);

  // Cleanup pending debounce on unmount
  useEffect(() => {
    return () => cancelDebounce();
  }, [cancelDebounce]);

  // Sync input value when the Zustand store is cleared externally (e.g. pressing
  // Escape or clicking the clear button).  This is an intentional synchronisation
  // between external state and local component state.
  useEffect(() => {
    if (!searchActive && searchQuery === "") {
      setInputValue(""); // eslint-disable-line react-hooks/set-state-in-effect -- syncing external store -> local state
    }
  }, [searchActive, searchQuery]);

  return (
    <div className="flex shrink-0 flex-col border-b border-border">
      <div className="flex items-center gap-2 px-3 py-1.5">
        <div className="relative flex flex-1 items-center">
          <Search className="pointer-events-none absolute left-2 size-4 text-muted-foreground" />
          <input
            ref={inputRef}
            type="text"
            value={inputValue}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            onBlur={handleBlur}
            placeholder="Search mail... (Ctrl+K)"
            data-search-input
            className="h-8 w-full rounded-md border border-border bg-background py-1 pl-8 pr-14 text-sm placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
          />
          <div className="absolute right-2 flex items-center gap-1">
            {inputValue && (
              <button
                type="button"
                onClick={handleClear}
                aria-label="Clear search"
                className="flex size-4 items-center justify-center rounded-sm text-muted-foreground hover:text-foreground"
              >
                <X className="size-3.5" />
              </button>
            )}
            <button
              type="button"
              onMouseDown={() => setShowTips(true)}
              onMouseUp={() => setShowTips(false)}
              onMouseLeave={() => setShowTips(false)}
              aria-label="Search tips (hold to show)"
              className="flex size-4 items-center justify-center rounded-sm text-muted-foreground hover:text-foreground"
            >
              <HelpCircle className="size-3.5" />
            </button>
          </div>

          {/* Search tips popover */}
          {showTips && (
            <div
              className="absolute left-0 top-full z-50 mt-1 w-full rounded-md border border-border bg-popover p-2 shadow-md"
            >
              <p className="mb-1.5 text-xs font-medium text-foreground">
                Search operators
              </p>
              <div className="space-y-1">
                {SEARCH_TIPS.map((tip) => (
                  <button
                    key={tip.operator}
                    type="button"
                    onClick={() => handleTipClick(tip.operator)}
                    className="flex w-full items-center gap-2 rounded px-1.5 py-1 text-left text-xs transition-colors hover:bg-muted"
                  >
                    <code className="shrink-0 rounded bg-muted px-1 py-0.5 font-mono text-[11px] text-primary">
                      {tip.operator}
                    </code>
                    <span className="min-w-0 flex-1 truncate text-muted-foreground">
                      {tip.desc}
                    </span>
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
        {searchActive && data && (
          <span className="shrink-0 text-xs text-muted-foreground">
            {data.total_count} result{data.total_count !== 1 ? "s" : ""}
          </span>
        )}
      </div>

      {/* Filter chips */}
      {parsed.filters.length > 0 && inputValue && (
        <div className="flex flex-wrap gap-1 px-3 pb-1.5">
          {parsed.filters.map((filter, idx) => (
            <span
              key={`${filter.operator}-${idx}`}
              className="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2 py-0.5 text-xs text-primary"
            >
              {getFilterLabel(filter)}
              <button
                type="button"
                onClick={() => handleRemoveFilter(filter.raw)}
                aria-label={`Remove ${filter.operator} filter`}
                className="flex size-3.5 items-center justify-center rounded-full transition-colors hover:bg-primary/20"
              >
                <X className="size-2.5" />
              </button>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
