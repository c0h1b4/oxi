"use client";

import { useCallback, useRef } from "react";
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
    </div>
  );
}

export function SearchResults() {
  const searchQuery = useUiStore((s) => s.searchQuery);
  const setActiveFolder = useUiStore((s) => s.setActiveFolder);
  const selectMessage = useUiStore((s) => s.selectMessage);
  const activeFolder = useUiStore((s) => s.activeFolder);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);

  const {
    data,
    isLoading,
    isError,
  } = useSearch(searchQuery);

  const scrollRef = useRef<HTMLDivElement>(null);

  const results = data?.results ?? [];
  const totalCount = data?.total_count ?? 0;

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
      {!isLoading && !isError && results.length === 0 && (
        <div className="flex flex-1 items-center justify-center px-4 text-center">
          <p className="text-sm text-muted-foreground">No results found</p>
        </div>
      )}

      {/* Results list with infinite scroll */}
      {!isLoading && !isError && results.length > 0 && (
        <>
          {/* Result count header */}
          <div className="flex shrink-0 items-center justify-between border-b border-border px-3 py-1">
            <span className="text-xs text-muted-foreground">
              {results.length < totalCount
                ? `Showing ${results.length} of ${totalCount} results`
                : `${totalCount} result${totalCount !== 1 ? "s" : ""}`}
            </span>
          </div>

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
        </>
      )}
    </div>
  );
}
