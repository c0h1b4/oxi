"use client";

import { useCallback } from "react";
import { Loader2, Paperclip } from "lucide-react";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/useUiStore";
import { useSearch } from "@/hooks/useSearch";
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
  onClick,
}: {
  result: SearchResultItem;
  onClick: () => void;
}) {
  const sender = result.from_name || result.from_address;
  const formattedDate = formatDate(result.date);

  return (
    <div
      role="row"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      className={cn(
        "flex cursor-pointer flex-col gap-0.5 border-b border-border px-3 py-2 transition-colors",
        "hover:bg-muted",
      )}
    >
      {/* Top row: sender, folder badge, date */}
      <div className="flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-sm font-medium">
          {sender}
        </span>
        <span className="shrink-0 rounded bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
          {result.folder}
        </span>
        <span className="shrink-0 text-xs text-muted-foreground">
          {formattedDate}
        </span>
      </div>

      {/* Subject + attachment */}
      <div className="flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-sm">{result.subject}</span>
        {result.has_attachments && (
          <Paperclip className="size-3.5 shrink-0 text-muted-foreground" />
        )}
      </div>

      {/* Snippet */}
      {result.snippet && (
        <p className="truncate text-xs text-muted-foreground">
          {result.snippet}
        </p>
      )}
    </div>
  );
}

export function SearchResults() {
  const searchQuery = useUiStore((s) => s.searchQuery);
  const clearSearch = useUiStore((s) => s.clearSearch);
  const setActiveFolder = useUiStore((s) => s.setActiveFolder);
  const selectMessage = useUiStore((s) => s.selectMessage);

  const { data, isLoading, isError } = useSearch(searchQuery);

  const handleResultClick = useCallback(
    (result: SearchResultItem) => {
      clearSearch();
      setActiveFolder(result.folder);
      selectMessage(result.uid);
    },
    [clearSearch, setActiveFolder, selectMessage],
  );

  return (
    <div className="flex h-full flex-col">
      {/* Loading state */}
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
      {!isLoading && !isError && data && data.results.length === 0 && (
        <div className="flex flex-1 items-center justify-center px-4 text-center">
          <p className="text-sm text-muted-foreground">No results found</p>
        </div>
      )}

      {/* Results list */}
      {!isLoading && !isError && data && data.results.length > 0 && (
        <div className="flex-1 overflow-y-auto">
          {data.results.map((result) => (
            <SearchResultRow
              key={`${result.folder}-${result.uid}`}
              result={result}
              onClick={() => handleResultClick(result)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
