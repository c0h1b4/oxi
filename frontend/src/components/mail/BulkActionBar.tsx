"use client";

import { useState, useRef, useCallback } from "react";
import {
  Mail,
  MailOpen,
  Star,
  Trash2,
  FolderInput,
  Tag,
  X,
  Loader2,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/useUiStore";
import {
  useUpdateFlags,
  useMoveMessage,
  useDeleteMessage,
} from "@/hooks/useMessages";
import { useFolders } from "@/hooks/useFolders";
import { useClickOutside } from "@/hooks/useClickOutside";
import { useTags, useBulkAddTag } from "@/hooks/useTags";

export function BulkActionBar() {
  const selectedUids = useUiStore((s) => s.selectedMessageUids);
  const activeFolder = useUiStore((s) => s.activeFolder);
  const clearBulkSelection = useUiStore((s) => s.clearBulkSelection);

  const updateFlags = useUpdateFlags();
  const moveMessage = useMoveMessage();
  const deleteMessage = useDeleteMessage();

  const [isBusy, setIsBusy] = useState(false);
  const [moveMenuOpen, setMoveMenuOpen] = useState(false);
  const [tagMenuOpen, setTagMenuOpen] = useState(false);
  const moveMenuRef = useRef<HTMLDivElement>(null);
  const tagMenuRef = useRef<HTMLDivElement>(null);

  const { data: foldersData } = useFolders();
  const folders = foldersData?.folders ?? [];
  const { data: tagsData } = useTags();
  const allTags = tagsData?.tags ?? [];
  const bulkAddTag = useBulkAddTag();

  const closeMoveMenu = useCallback(() => setMoveMenuOpen(false), []);
  const closeTagMenu = useCallback(() => setTagMenuOpen(false), []);
  useClickOutside(moveMenuRef, closeMoveMenu, moveMenuOpen);
  useClickOutside(tagMenuRef, closeTagMenu, tagMenuOpen);

  const runBulkAction = useCallback(
    async (
      action: (uid: number) => Promise<unknown>,
    ) => {
      setIsBusy(true);
      try {
        await Promise.allSettled(selectedUids.map(action));
      } finally {
        setIsBusy(false);
        clearBulkSelection();
      }
    },
    [selectedUids, clearBulkSelection],
  );

  const handleMarkRead = useCallback(() => {
    runBulkAction((uid) =>
      updateFlags.mutateAsync({
        folder: activeFolder,
        uid,
        flags: ["\\Seen"],
        add: true,
      }),
    );
  }, [runBulkAction, updateFlags, activeFolder]);

  const handleMarkUnread = useCallback(() => {
    runBulkAction((uid) =>
      updateFlags.mutateAsync({
        folder: activeFolder,
        uid,
        flags: ["\\Seen"],
        add: false,
      }),
    );
  }, [runBulkAction, updateFlags, activeFolder]);

  const handleStar = useCallback(() => {
    runBulkAction((uid) =>
      updateFlags.mutateAsync({
        folder: activeFolder,
        uid,
        flags: ["\\Flagged"],
        add: true,
      }),
    );
  }, [runBulkAction, updateFlags, activeFolder]);

  const handleDelete = useCallback(() => {
    runBulkAction((uid) =>
      deleteMessage.mutateAsync({ folder: activeFolder, uid }),
    );
  }, [runBulkAction, deleteMessage, activeFolder]);

  const handleMoveTo = useCallback(
    (targetFolder: string) => {
      setMoveMenuOpen(false);
      runBulkAction((uid) =>
        moveMessage.mutateAsync({
          fromFolder: activeFolder,
          toFolder: targetFolder,
          uid,
        }),
      );
    },
    [runBulkAction, moveMessage, activeFolder],
  );

  const handleBulkTag = useCallback(
    (tagId: string) => {
      setTagMenuOpen(false);
      bulkAddTag.mutate({
        tagId,
        messages: selectedUids.map((uid) => ({ uid, folder: activeFolder })),
      });
    },
    [bulkAddTag, selectedUids, activeFolder],
  );

  if (selectedUids.length < 1) return null;

  return (
    <div className="flex shrink-0 items-center gap-1 border-b border-border bg-muted/50 px-3 py-1.5">
      {/* Selection count */}
      <span className="mr-2 text-sm font-medium text-foreground">
        {selectedUids.length} selected
      </span>

      {isBusy && (
        <Loader2 className="mr-1 size-4 animate-spin text-muted-foreground" />
      )}

      {/* Mark read */}
      <button
        type="button"
        title="Mark as read"
        disabled={isBusy}
        onClick={handleMarkRead}
        className={cn(
          "rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground",
          "disabled:pointer-events-none disabled:opacity-50",
        )}
      >
        <Mail className="size-4" />
      </button>

      {/* Mark unread */}
      <button
        type="button"
        title="Mark as unread"
        disabled={isBusy}
        onClick={handleMarkUnread}
        className={cn(
          "rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground",
          "disabled:pointer-events-none disabled:opacity-50",
        )}
      >
        <MailOpen className="size-4" />
      </button>

      {/* Star */}
      <button
        type="button"
        title="Star"
        disabled={isBusy}
        onClick={handleStar}
        className={cn(
          "rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground",
          "disabled:pointer-events-none disabled:opacity-50",
        )}
      >
        <Star className="size-4" />
      </button>

      {/* Delete */}
      <button
        type="button"
        title="Delete"
        disabled={isBusy}
        onClick={handleDelete}
        className={cn(
          "rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-destructive",
          "disabled:pointer-events-none disabled:opacity-50",
        )}
      >
        <Trash2 className="size-4" />
      </button>

      {/* Move to folder */}
      <div className="relative" ref={moveMenuRef}>
        <button
          type="button"
          title="Move to folder"
          disabled={isBusy}
          onClick={() => setMoveMenuOpen((prev) => !prev)}
          className={cn(
            "rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground",
            "disabled:pointer-events-none disabled:opacity-50",
            moveMenuOpen && "bg-accent text-foreground",
          )}
        >
          <FolderInput className="size-4" />
        </button>

        {moveMenuOpen && (
          <div className="absolute left-0 top-full z-50 mt-1 min-w-[160px] rounded-md border border-border bg-popover py-1 shadow-md">
            {folders
              .filter((f) => f.name !== activeFolder)
              .map((f) => (
                <button
                  key={f.name}
                  type="button"
                  onClick={() => handleMoveTo(f.name)}
                  className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-accent"
                >
                  {f.name}
                </button>
              ))}
            {folders.filter((f) => f.name !== activeFolder).length === 0 && (
              <span className="block px-3 py-1.5 text-sm text-muted-foreground">
                No other folders
              </span>
            )}
          </div>
        )}
      </div>

      {/* Tag */}
      <div className="relative" ref={tagMenuRef}>
        <button
          type="button"
          title="Add tag"
          disabled={isBusy}
          onClick={() => setTagMenuOpen((prev) => !prev)}
          className={cn(
            "rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground",
            "disabled:pointer-events-none disabled:opacity-50",
            tagMenuOpen && "bg-accent text-foreground",
          )}
        >
          <Tag className="size-4" />
        </button>

        {tagMenuOpen && (
          <div className="absolute left-0 top-full z-50 mt-1 min-w-[160px] rounded-md border border-border bg-popover py-1 shadow-md">
            {allTags.map((tag) => (
              <button
                key={tag.id}
                type="button"
                onClick={() => handleBulkTag(tag.id)}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-accent"
              >
                <span
                  className="size-2.5 shrink-0 rounded-full"
                  style={{ backgroundColor: tag.color }}
                />
                {tag.name}
              </button>
            ))}
            {allTags.length === 0 && (
              <span className="block px-3 py-1.5 text-sm text-muted-foreground">
                No tags
              </span>
            )}
          </div>
        )}
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Clear selection */}
      <button
        type="button"
        title="Clear selection"
        onClick={clearBulkSelection}
        className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
      >
        <X className="size-4" />
      </button>
    </div>
  );
}
