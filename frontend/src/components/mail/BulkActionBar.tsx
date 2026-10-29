"use client";

import { useState, useCallback, useMemo } from "react";
import { Popover } from "radix-ui";
import { AnimatePresence } from "framer-motion";
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
import { useTags, useBulkAddTag } from "@/hooks/useTags";
import { createFadeSlideVariants } from "@/lib/motion/variants";
import { AnimatedDiv } from "@/lib/motion/AnimatedDiv";

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

  const { data: foldersData } = useFolders();
  const folders = foldersData?.folders ?? [];
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const barMotionProps = useMemo(() => createFadeSlideVariants(effectiveAnimationMode, "y"), [effectiveAnimationMode]);
  const { data: tagsData } = useTags();
  const allTags = tagsData?.tags ?? [];
  const bulkAddTag = useBulkAddTag();


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

  const hasSelection = selectedUids.length > 0;
  const actionContent = (
    <>
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
      <Popover.Root open={moveMenuOpen} onOpenChange={setMoveMenuOpen}>
        <Popover.Trigger asChild>
        <button
          type="button"
          title="Move to folder"
          disabled={isBusy}
          className={cn(
            "rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground",
            "disabled:pointer-events-none disabled:opacity-50",
            moveMenuOpen && "bg-accent text-foreground",
          )}
        >
          <FolderInput className="size-4" />
        </button>
        </Popover.Trigger>

        {moveMenuOpen && (
          <Popover.Portal><Popover.Content sideOffset={4} collisionPadding={8} className="z-50 max-h-[var(--radix-popover-content-available-height)] w-48 max-w-[calc(100vw-1rem)] overflow-y-auto rounded-md border border-border bg-popover py-1 shadow-md [overflow-wrap:anywhere]">
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
          </Popover.Content></Popover.Portal>
        )}
      </Popover.Root>

      {/* Tag */}
      <Popover.Root open={tagMenuOpen} onOpenChange={setTagMenuOpen}>
        <Popover.Trigger asChild>
        <button
          type="button"
          title="Add tag"
          disabled={isBusy}
          className={cn(
            "rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground",
            "disabled:pointer-events-none disabled:opacity-50",
            tagMenuOpen && "bg-accent text-foreground",
          )}
        >
          <Tag className="size-4" />
        </button>
        </Popover.Trigger>

        {tagMenuOpen && (
          <Popover.Portal><Popover.Content sideOffset={4} collisionPadding={8} className="z-50 max-h-[var(--radix-popover-content-available-height)] w-48 max-w-[calc(100vw-1rem)] overflow-y-auto rounded-md border border-border bg-popover py-1 shadow-md [overflow-wrap:anywhere]">
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
          </Popover.Content></Popover.Portal>
        )}
      </Popover.Root>

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
    </>
  );

  return (
    <AnimatePresence>
      {hasSelection ? (
        <AnimatedDiv
          key="bulk-action-bar"
          data-testid="bulk-action-bar-transition"
          variants={barMotionProps}
          initial="initial"
          animate="animate"
          exit="exit"
          className="flex shrink-0 flex-wrap items-center gap-1 border-b border-border bg-muted/50 px-3 py-1.5 [@media(pointer:coarse)]:[&_button]:min-h-11 [@media(pointer:coarse)]:[&_button]:min-w-11"
        >
          {actionContent}
        </AnimatedDiv>
      ) : null}
    </AnimatePresence>
  );
}
