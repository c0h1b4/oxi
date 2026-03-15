"use client";

import { useCallback, useEffect, useRef } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { ArrowDown, ArrowUp, Loader2, Paperclip, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/useUiStore";
import { useSearch } from "@/hooks/useSearch";
import {
  getFilterLabel,
  isValidCommittedSearch,
  normalizeSearchQuery,
  parseSearchQuery,
  removeFilterFromQuery,
} from "@/lib/search-parser";
import type { SearchResultItem } from "@/types/message";

function formatDate(dateStr: string): string {
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return dateStr;

  const now = new Date();
  const isToday =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();

  if (isToday) {
    return date.toLocaleTimeString(undefined, {
      hour: "numeric",
      minute: "2-digit",
    });
  }

  const msPerDay = 86_400_000;
  const daysDiff = Math.floor((now.getTime() - date.getTime()) / msPerDay);
  const time = date.toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });

  if (daysDiff < 7 && daysDiff >= 0) {
    const day = date.toLocaleDateString(undefined, { weekday: "short" });
    return `${day} ${time}`;
  }

  if (date.getFullYear() === now.getFullYear()) {
    const day = date.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
    return `${day} ${time}`;
  }

  const day = date.toLocaleDateString(undefined, {
    month: "2-digit",
    day: "2-digit",
    year: "2-digit",
  });
  return `${day} ${time}`;
}

function SearchResultRow({
  result,
  isSelected,
  onClick,
}: {
  result: SearchResultItem;
  isSelected: boolean;
  onClick: () => void;
}) {
  const sender = result.from_name || result.from_address;
  const formattedDate = formatDate(result.date);
  const isUnread = !result.flags.includes("\\Seen");
  const isFlagged = result.flags.includes("\\Flagged");

  return (
    <button
      type="button"
      onClick={onClick}
      data-search-result-folder={result.folder}
      data-search-result-uid={result.uid}
      className={cn(
        "flex w-full cursor-pointer flex-col gap-0.5 border-b border-border px-3 py-2 text-left transition-colors",
        "hover:bg-muted",
        isUnread ? "bg-background" : "bg-transparent",
        isSelected && "bg-accent hover:bg-accent",
      )}
    >
      {/* Top row: unread dot, sender, folder badge, date */}
      <div className="flex items-center gap-2">
        <span
          className={cn(
            "size-1.5 shrink-0 rounded-full",
            isUnread ? "bg-primary" : "bg-transparent",
          )}
        />
        <span className={cn(
          "min-w-0 flex-1 truncate text-sm",
          isUnread ? "font-semibold" : "font-medium",
          isFlagged ? "text-primary" : "text-foreground",
        )}>
          {sender}
        </span>
        <span className="shrink-0 rounded bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
          {result.folder}
        </span>
        <span className={cn("shrink-0 text-xs", isFlagged ? "text-primary" : "text-muted-foreground")}>
          {formattedDate}
        </span>
      </div>

      {/* Subject + attachment */}
      <div className="flex items-center gap-2 pl-3.5">
        <span className={cn(
          "min-w-0 flex-1 truncate text-sm",
          isUnread ? "font-medium" : "font-normal",
          isFlagged ? "text-primary" : "text-foreground",
        )}>{result.subject || "(no subject)"}</span>
        {result.has_attachments && (
          <Paperclip className="size-3.5 shrink-0 text-muted-foreground" />
        )}
      </div>

      {/* Snippet */}
      {result.snippet && (
        <p className="truncate pl-3.5 text-xs text-muted-foreground">
          {result.snippet}
        </p>
      )}
    </button>
  );
}

export function SearchResults() {
  const searchQuery = useUiStore((s) => s.searchQuery);
  const setSearchQuery = useUiStore((s) => s.setSearchQuery);
  const setSearchActive = useUiStore((s) => s.setSearchActive);
  const setActiveFolder = useUiStore((s) => s.setActiveFolder);
  const selectMessage = useUiStore((s) => s.selectMessage);
  const activeFolder = useUiStore((s) => s.activeFolder);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const searchSortOrder = useUiStore((s) => s.searchSortOrder);
  const setSearchSortOrder = useUiStore((s) => s.setSearchSortOrder);
  const shouldAnimate = effectiveAnimationMode !== "off";

  const listTransition = {
    initial: { opacity: 0, y: 6 },
    animate: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.22, ease: [0.2, 0, 0, 1] as const },
    },
    exit: {
      opacity: 0,
      y: 3,
      transition: { duration: 0.14, ease: [0.2, 0, 0, 1] as const },
    },
  };

  const itemTransition = {
    initial: { opacity: 0, x: 6 },
    animate: {
      opacity: 1,
      x: 0,
      transition: { duration: 0.18, ease: [0.2, 0, 0, 1] as const },
    },
    exit: {
      opacity: 0,
      x: -3,
      transition: { duration: 0.1, ease: [0.2, 0, 0, 1] as const },
    },
  };

  const normalizedSearchQuery = normalizeSearchQuery(searchQuery);
  const hasValidCommittedSearch = isValidCommittedSearch(normalizedSearchQuery);

  const {
    data,
    isLoading,
    isError,
  } = useSearch(searchQuery, undefined, searchSortOrder);

  // Parse filters for display in the results header
  const parsed = parseSearchQuery(normalizedSearchQuery);

  const handleRemoveFilter = useCallback(
    (filterRaw: string) => {
      const nextQuery = normalizeSearchQuery(removeFilterFromQuery(searchQuery, filterRaw));
      const hasValidNextQuery = isValidCommittedSearch(nextQuery);
      setSearchQuery(hasValidNextQuery ? nextQuery : "");
      setSearchActive(hasValidNextQuery);
    },
    [searchQuery, setSearchQuery, setSearchActive],
  );

  const scrollRef = useRef<HTMLDivElement>(null);
  const prevSelectionKeyRef = useRef<string | null>(null);

  const results = data?.results ?? [];
  const totalCount = data?.total_count ?? 0;

  useEffect(() => {
    if (selectedMessageUid == null || results.length === 0) return;

    const selectionKey = `${activeFolder}:${selectedMessageUid}`;
    if (selectionKey === prevSelectionKeyRef.current) return;

    const scrollEl = scrollRef.current;
    if (!scrollEl) return;

    const selectedRow = scrollEl.querySelector(
      `[data-search-result-folder="${activeFolder}"][data-search-result-uid="${selectedMessageUid}"]`,
    ) as HTMLElement | null;
    if (!selectedRow) return;

    prevSelectionKeyRef.current = selectionKey;

    const scrollRect = scrollEl.getBoundingClientRect();
    const rowRect = selectedRow.getBoundingClientRect();
    const rowTop = rowRect.top - scrollRect.top + scrollEl.scrollTop;
    const rowBottom = rowTop + rowRect.height;
    const viewTop = scrollEl.scrollTop;
    const viewBottom = viewTop + scrollEl.clientHeight;
    const buffer = rowRect.height * 3;

    if (rowTop < viewTop + buffer) {
      scrollEl.scrollTop = Math.max(0, rowTop - buffer);
    } else if (rowBottom > viewBottom - buffer) {
      scrollEl.scrollTop = rowBottom - scrollEl.clientHeight + buffer;
    }
  }, [selectedMessageUid, activeFolder, results.length]);

  const handleResultClick = useCallback(
    (result: SearchResultItem) => {
      setActiveFolder(result.folder);
      selectMessage(result.uid);
    },
    [setActiveFolder, selectMessage],
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* Loading state (initial load only) */}
      {isLoading && (
        <div className="flex flex-1 items-center justify-center">
          <Loader2 className="size-6 animate-spin text-muted-foreground" />
        </div>
      )}

      {/* Error state */}
      {isError && (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 px-4 text-center">
          <p className="text-sm text-muted-foreground">
            Failed to load search results
          </p>
        </div>
      )}

      {/* Empty state */}
      {!isLoading && !isError && hasValidCommittedSearch && results.length === 0 && (
        <div className="flex flex-1 items-center justify-center px-4 pt-4 text-center">
          <p className="text-sm text-muted-foreground">No results found</p>
        </div>
      )}

      {/* Results list with infinite scroll */}
      {!isLoading && !isError && hasValidCommittedSearch && (
        <>
          {/* Result count header */}
          {results.length > 0 && (
            <div className="shrink-0 border-b border-border px-3 py-1">
              <div className="flex items-center justify-between">
                <span className="text-xs text-muted-foreground">
                  {results.length < totalCount
                    ? `Showing ${results.length} of ${totalCount} results`
                    : `${totalCount} result${totalCount !== 1 ? "s" : ""}`}
                </span>
                <button
                  type="button"
                  onClick={() => setSearchSortOrder(searchSortOrder === "date_desc" ? "date_asc" : "date_desc")}
                  className="flex items-center gap-1 rounded px-1.5 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                  title={searchSortOrder === "date_desc" ? "Newest first" : "Oldest first"}
                >
                  {searchSortOrder === "date_desc" ? (
                    <ArrowDown className="size-3" />
                  ) : (
                    <ArrowUp className="size-3" />
                  )}
                  Date
                </button>
              </div>
              {parsed.filters.length > 0 && (
                <div className="mt-1 flex flex-wrap gap-1">
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
          )}

          {shouldAnimate ? (
            <AnimatePresence initial={false}>
              {results.length > 0 && (
                <motion.div
                  key="search-results-list"
                  ref={scrollRef}
                  data-testid="search-results-list-transition"
                  data-motion-props={JSON.stringify(listTransition)}
                  initial={listTransition.initial}
                  animate={listTransition.animate}
                  exit={listTransition.exit}
                  className="min-h-0 flex-1 overflow-y-auto"
                >
                  <AnimatePresence initial={false}>
                    {results.map((result) => (
                      <motion.div
                        key={`${result.folder}-${result.uid}`}
                        data-testid="search-results-item-transition"
                        data-motion-props={JSON.stringify(itemTransition)}
                        initial={itemTransition.initial}
                        animate={itemTransition.animate}
                        exit={itemTransition.exit}
                      >
                        <SearchResultRow
                          result={result}
                          isSelected={
                            activeFolder === result.folder &&
                            selectedMessageUid === result.uid
                          }
                          onClick={() => handleResultClick(result)}
                        />
                      </motion.div>
                    ))}
                  </AnimatePresence>
                </motion.div>
              )}
            </AnimatePresence>
          ) : (
            results.length > 0 && (
              <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto">
                {results.map((result) => (
                  <SearchResultRow
                    key={`${result.folder}-${result.uid}`}
                    result={result}
                    isSelected={
                      activeFolder === result.folder &&
                      selectedMessageUid === result.uid
                    }
                    onClick={() => handleResultClick(result)}
                  />
                ))}
              </div>
            )
          )}
        </>
      )}
    </div>
  );
}
