"use client";

import { useRef, useCallback } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useMessages } from "@/hooks/useMessages";
import { useUiStore } from "@/stores/useUiStore";
import { MessageListItem } from "./MessageListItem";
import { Button } from "@/components/ui/button";

function SkeletonRows({ count, height }: { count: number; height: number }) {
  return (
    <div className="flex flex-col">
      {Array.from({ length: count }).map((_, i) => (
        <div
          key={i}
          className="flex items-center gap-3 border-b border-border px-3"
          style={{ height }}
        >
          <div className="h-3 w-3 animate-pulse rounded-full bg-muted" />
          <div className="h-3 w-24 animate-pulse rounded bg-muted" />
          <div className="h-3 flex-1 animate-pulse rounded bg-muted" />
          <div className="h-3 w-12 animate-pulse rounded bg-muted" />
        </div>
      ))}
    </div>
  );
}

export function MessageList() {
  const activeFolder = useUiStore((s) => s.activeFolder);
  const density = useUiStore((s) => s.density);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const selectMessage = useUiStore((s) => s.selectMessage);

  const { data, isLoading, isError, refetch } = useMessages(activeFolder);

  const messages = data?.messages ?? [];
  const totalCount = data?.total_count ?? 0;

  const parentRef = useRef<HTMLDivElement>(null);
  const rowHeight = density === "compact" ? 36 : 64;

  const virtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => rowHeight,
    overscan: 10,
  });

  const handleClick = useCallback(
    (uid: number) => {
      selectMessage(uid);
    },
    [selectMessage],
  );

  return (
    <div className="flex h-full flex-col">
      {/* Header bar */}
      <div className="flex shrink-0 items-center justify-between border-b border-border px-4 py-2">
        <h2 className="text-sm font-semibold">{activeFolder}</h2>
        <span className="text-xs text-muted-foreground">
          {isLoading ? "\u2026" : `${totalCount} messages`}
        </span>
      </div>

      {/* Loading state */}
      {isLoading && (
        <SkeletonRows count={8} height={rowHeight} />
      )}

      {/* Error state */}
      {isError && (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 px-4 py-8 text-center">
          <p className="text-sm text-muted-foreground">
            Failed to load messages
          </p>
          <Button variant="outline" size="sm" onClick={() => refetch()}>
            Retry
          </Button>
        </div>
      )}

      {/* Empty state */}
      {!isLoading && !isError && messages.length === 0 && (
        <div className="flex flex-1 items-center justify-center text-muted-foreground">
          No messages in this folder
        </div>
      )}

      {/* Virtualized message list */}
      {!isLoading && !isError && messages.length > 0 && (
        <div ref={parentRef} className="flex-1 overflow-y-auto">
          <div
            style={{
              height: virtualizer.getTotalSize(),
              width: "100%",
              position: "relative",
            }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const message = messages[virtualRow.index];
              return (
                <div
                  key={message.uid}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: virtualRow.size,
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <MessageListItem
                    message={message}
                    isSelected={selectedMessageUid === message.uid}
                    density={density}
                    onClick={() => handleClick(message.uid)}
                  />
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
