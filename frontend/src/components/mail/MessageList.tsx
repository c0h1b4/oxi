"use client";

import { useRef, useCallback, useEffect, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { PenLine, X } from "lucide-react";
import { useMessages } from "@/hooks/useMessages";
import { useListDrafts, useGetDraft, useDeleteDraft } from "@/hooks/useCompose";
import { useUiStore } from "@/stores/useUiStore";
import { useComposeStore } from "@/stores/useComposeStore";
import { MessageListItem } from "./MessageListItem";
import { formatFolderName, isDraftsFolder } from "./FolderTree";
import { BulkActionBar } from "./BulkActionBar";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

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

function humanizeDate(iso: string): string {
  const date = new Date(iso);
  if (isNaN(date.getTime())) return iso;

  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMinutes = Math.floor(diffMs / 60_000);
  const diffHours = Math.floor(diffMs / 3_600_000);

  if (diffMinutes < 1) return "just now";
  if (diffMinutes < 60) {
    return diffMinutes === 1 ? "1 minute ago" : `${diffMinutes} minutes ago`;
  }
  if (diffHours < 24) {
    return diffHours === 1 ? "1 hour ago" : `${diffHours} hours ago`;
  }

  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (
    date.getFullYear() === yesterday.getFullYear() &&
    date.getMonth() === yesterday.getMonth() &&
    date.getDate() === yesterday.getDate()
  ) {
    return "yesterday";
  }

  return date.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });
}

function DraftItems() {
  const { data } = useListDrafts(true);
  const deleteDraft = useDeleteDraft();
  const openDraft = useComposeStore((s) => s.openDraft);
  const isComposeOpen = useComposeStore((s) => s.isOpen);
  const [loadingDraftId, setLoadingDraftId] = useState<string | null>(null);
  const getDraft = useGetDraft(loadingDraftId);

  // When draft data arrives, open it in the compose dialog.
  useEffect(() => {
    if (!getDraft.data || !loadingDraftId) return;
    const d = getDraft.data;
    openDraft({
      id: d.id,
      to: d.to,
      cc: d.cc,
      bcc: d.bcc,
      subject: d.subject,
      body: d.html_body ?? d.text_body,
      inReplyTo: d.in_reply_to,
      references: d.references,
      attachments: d.attachments.map((a) => ({
        id: a.id,
        filename: a.filename,
        contentType: a.content_type,
        size: a.size,
      })),
    });
    setLoadingDraftId(null); // eslint-disable-line react-hooks/set-state-in-effect -- clearing after zustand store update
  }, [getDraft.data, loadingDraftId, openDraft]);

  const drafts = data?.drafts ?? [];
  if (drafts.length === 0) return null;

  return (
    <>
      {drafts.map((draft) => (
        <div
          key={draft.id}
          role="row"
          tabIndex={0}
          onClick={() => {
            if (!isComposeOpen) setLoadingDraftId(draft.id);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              if (!isComposeOpen) setLoadingDraftId(draft.id);
            }
          }}
          className={cn(
            "group flex h-16 cursor-pointer flex-col justify-center border-b border-border px-3 py-1.5 transition-colors",
            "hover:bg-muted bg-transparent",
          )}
        >
          {/* Top row: "Draft" label + date + delete */}
          <div className="flex items-center gap-2">
            <PenLine className="size-3.5 shrink-0 text-muted-foreground" />
            <span className="min-w-0 flex-1 truncate text-xs font-normal text-muted-foreground">
              {draft.to || "No recipient"}
            </span>
            <span className="shrink-0 text-xs font-normal text-muted-foreground">
              {humanizeDate(draft.updated_at)}
            </span>
            <button
              onClick={(e) => {
                e.stopPropagation();
                deleteDraft.mutate(draft.id);
              }}
              className="hidden shrink-0 rounded p-0.5 text-muted-foreground hover:bg-accent hover:text-foreground group-hover:flex items-center justify-center"
              title="Delete draft"
            >
              <X className="size-3" />
            </button>
          </div>

          {/* Bottom row: subject */}
          <div className="flex items-center gap-2">
            <span className="min-w-0 flex-1 truncate text-sm font-normal text-foreground">
              {draft.subject || "(no subject)"}
            </span>
          </div>
        </div>
      ))}
    </>
  );
}

export function MessageList() {
  const activeFolder = useUiStore((s) => s.activeFolder);
  const density = useUiStore((s) => s.density);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const selectMessage = useUiStore((s) => s.selectMessage);
  const showDrafts = isDraftsFolder(activeFolder);
  const selectedMessageUids = useUiStore((s) => s.selectedMessageUids);
  const bulkSelectMode = useUiStore((s) => s.bulkSelectMode);
  const toggleBulkSelect = useUiStore((s) => s.toggleBulkSelect);
  const selectAllMessages = useUiStore((s) => s.selectAllMessages);
  const clearBulkSelection = useUiStore((s) => s.clearBulkSelection);

  const {
    data,
    isLoading,
    isError,
    refetch,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
  } = useMessages(activeFolder);

  // Flatten all pages into a single array of messages.
  const messages = data?.pages.flatMap((page) => page.messages) ?? [];
  const totalCount = data?.pages[0]?.total_count ?? 0;

  const parentRef = useRef<HTMLDivElement>(null);
  const rowHeight = density === "compact" ? 36 : 64;

  // eslint-disable-next-line react-hooks/incompatible-library -- useVirtualizer is designed for this usage
  const virtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => rowHeight,
    overscan: 10,
  });

  // Fetch next page when scrolling near the bottom.
  const virtualItems = virtualizer.getVirtualItems();
  const lastItem = virtualItems[virtualItems.length - 1];
  const lastItemIndex = lastItem?.index;

  useEffect(() => {
    if (lastItemIndex == null) return;
    if (lastItemIndex >= messages.length - 10 && hasNextPage && !isFetchingNextPage) {
      fetchNextPage();
    }
  }, [lastItemIndex, messages.length, hasNextPage, isFetchingNextPage, fetchNextPage]);

  // Anchor UID for shift-click range selection.
  const anchorUidRef = useRef<number | null>(null);

  const handleClick = useCallback(
    (uid: number, e: React.MouseEvent) => {
      const isMod = e.metaKey || e.ctrlKey;
      const isShift = e.shiftKey;

      if (isMod) {
        // Cmd/Ctrl+click: toggle this message in bulk selection.
        // If entering bulk mode while reading a message, include the read message too.
        if (selectedMessageUids.length === 0 && selectedMessageUid != null && selectedMessageUid !== uid) {
          selectAllMessages([selectedMessageUid, uid]);
        } else {
          toggleBulkSelect(uid);
        }
        anchorUidRef.current = uid;
        return;
      }

      if (isShift && anchorUidRef.current != null) {
        // Shift+click: select range from anchor to clicked message.
        const uids = messages.map((m) => m.uid);
        const anchorIdx = uids.indexOf(anchorUidRef.current);
        const clickedIdx = uids.indexOf(uid);
        if (anchorIdx !== -1 && clickedIdx !== -1) {
          const start = Math.min(anchorIdx, clickedIdx);
          const end = Math.max(anchorIdx, clickedIdx);
          selectAllMessages(uids.slice(start, end + 1));
        }
        return;
      }

      // Plain click: normal single-message selection, clear bulk.
      clearBulkSelection();
      selectMessage(uid);
      anchorUidRef.current = uid;
    },
    [selectMessage, toggleBulkSelect, selectAllMessages, clearBulkSelection, messages, selectedMessageUid, selectedMessageUids],
  );

  const allUids = messages.map((m) => m.uid);
  const allSelected =
    messages.length > 0 && selectedMessageUids.length === messages.length;

  const handleSelectAllToggle = useCallback(() => {
    if (allSelected) {
      clearBulkSelection();
    } else {
      selectAllMessages(allUids);
    }
  }, [allSelected, allUids, clearBulkSelection, selectAllMessages]);

  return (
    <div className="flex h-full flex-col">
      {/* Header bar */}
      <div className="flex shrink-0 items-center justify-between border-b border-border px-4 py-2">
        <div className="flex items-center gap-2">
          {/* Select all checkbox */}
          {!isLoading && !isError && messages.length > 0 && (
            <button
              type="button"
              aria-label={allSelected ? "Deselect all" : "Select all"}
              onClick={handleSelectAllToggle}
              className={cn(
                "flex size-4 shrink-0 items-center justify-center rounded border transition-colors",
                allSelected
                  ? "border-primary bg-primary text-primary-foreground"
                  : "border-muted-foreground/40 bg-transparent hover:border-primary",
              )}
            >
              {allSelected && (
                <svg
                  className="size-3"
                  viewBox="0 0 12 12"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <path d="M2.5 6l2.5 2.5 4.5-5" />
                </svg>
              )}
            </button>
          )}
          <h2 className="text-sm font-semibold">{formatFolderName(activeFolder)}</h2>
        </div>
        <span className="text-xs text-muted-foreground">
          {isLoading ? "\u2026" : `${totalCount} messages`}
        </span>
      </div>

      {/* Bulk action bar */}
      <BulkActionBar />

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

      {/* Empty state (only when not a drafts folder or no drafts either) */}
      {!isLoading && !isError && messages.length === 0 && !showDrafts && (
        <div className="flex flex-1 items-center justify-center text-muted-foreground">
          No messages in this folder
        </div>
      )}

      {/* Scrollable content: drafts + virtualized message list */}
      {!isLoading && !isError && (showDrafts || messages.length > 0) && (
        <div ref={parentRef} className="flex-1 overflow-y-auto">
          {/* Local drafts at top of the Drafts folder */}
          {showDrafts && <DraftItems />}

          {/* Virtualized IMAP messages */}
          {messages.length > 0 && (
            <div
              style={{
                height: virtualizer.getTotalSize(),
                width: "100%",
                position: "relative",
              }}
            >
              {virtualItems.map((virtualRow) => {
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
                      onClick={(e) => handleClick(message.uid, e)}
                      bulkSelectMode={bulkSelectMode}
                      isBulkSelected={selectedMessageUids.includes(message.uid)}
                      onBulkToggle={toggleBulkSelect}
                    />
                  </div>
                );
              })}
            </div>
          )}
          {isFetchingNextPage && (
            <div className="flex items-center justify-center py-2">
              <span className="text-xs text-muted-foreground">Loading more...</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
