"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import { useUiStore } from "@/stores/useUiStore";
import { useSearch } from "@/hooks/useSearch";

export function SearchBar() {
  const searchQuery = useUiStore((s) => s.searchQuery);
  const searchActive = useUiStore((s) => s.searchActive);
  const setSearchQuery = useUiStore((s) => s.setSearchQuery);
  const setSearchActive = useUiStore((s) => s.setSearchActive);
  const clearSearch = useUiStore((s) => s.clearSearch);

  const [inputValue, setInputValue] = useState(searchQuery);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Fetch results for displaying the count
  const { data } = useSearch(searchQuery);

  // Debounce input changes before updating the store
  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const value = e.target.value;
      setInputValue(value);

      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }

      debounceRef.current = setTimeout(() => {
        setSearchQuery(value);
        setSearchActive(value.length >= 2);
      }, 300);
    },
    [setSearchQuery, setSearchActive],
  );

  // Clear search on Escape
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Escape") {
        setInputValue("");
        clearSearch();
        inputRef.current?.blur();
      }
    },
    [clearSearch],
  );

  // Clear button handler
  const handleClear = useCallback(() => {
    setInputValue("");
    clearSearch();
    inputRef.current?.focus();
  }, [clearSearch]);

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

  // Sync input value when store is cleared externally
  useEffect(() => {
    if (!searchActive && searchQuery === "") {
      setInputValue("");
    }
  }, [searchActive, searchQuery]);

  return (
    <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-1.5">
      <div className="relative flex flex-1 items-center">
        <Search className="pointer-events-none absolute left-2 size-4 text-muted-foreground" />
        <input
          ref={inputRef}
          type="text"
          value={inputValue}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          placeholder="Search mail... (Ctrl+K)"
          className="h-8 w-full rounded-md border border-border bg-background py-1 pl-8 pr-8 text-sm placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
        />
        {inputValue && (
          <button
            type="button"
            onClick={handleClear}
            aria-label="Clear search"
            className="absolute right-2 flex size-4 items-center justify-center rounded-sm text-muted-foreground hover:text-foreground"
          >
            <X className="size-3.5" />
          </button>
        )}
      </div>
      {searchActive && data && (
        <span className="shrink-0 text-xs text-muted-foreground">
          {data.total_count} result{data.total_count !== 1 ? "s" : ""}
        </span>
      )}
    </div>
  );
}
