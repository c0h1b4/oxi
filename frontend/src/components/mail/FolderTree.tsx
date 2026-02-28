"use client";

import { useState, useRef, useEffect, useCallback } from "react";
import {
  Inbox,
  Send,
  FileText,
  Trash2,
  AlertCircle,
  Star,
  Folder,
  Loader2,
  FolderPlus,
  Check,
  X,
} from "lucide-react";
import { useIsFetching } from "@tanstack/react-query";
import { useFolders, useRenameFolder } from "@/hooks/useFolders";
import { useMoveMessage, usePrefetchAllFolders } from "@/hooks/useMessages";
import { useUiStore } from "@/stores/useUiStore";
import { useAuthStore } from "@/stores/useAuthStore";
import { Button } from "@/components/ui/button";
import { FolderContextMenu } from "@/components/mail/FolderContextMenu";
import { CreateFolderDialog } from "@/components/mail/CreateFolderDialog";
import { cn } from "@/lib/utils";
import type { Folder as FolderType } from "@/types/folder";

/** Map raw folder names to user-friendly display names. */
export function formatFolderName(name: string): string {
  const lower = name.toLowerCase();
  if (lower === "inbox") return "Inbox";
  if (lower === "drafts" || lower === "[gmail]/drafts") return "Drafts";
  if (lower === "sent" || lower === "[gmail]/sent mail") return "Sent";
  if (lower === "trash" || lower === "[gmail]/trash") return "Trash";
  if (lower === "junk" || lower === "spam" || lower === "[gmail]/spam") return "Spam";
  if (lower === "archive" || lower === "[gmail]/all mail") return "Archive";
  if (lower === "starred" || lower === "flagged") return "Starred";
  return name;
}

/** Check whether a folder name refers to the Drafts folder. */
export function isDraftsFolder(name: string): boolean {
  const lower = name.toLowerCase();
  return lower === "drafts" || lower === "[gmail]/drafts";
}

/** Sort priority for well-known folders.  Lower = higher in the list. */
function folderSortOrder(name: string): number {
  const lower = name.toLowerCase();
  if (lower === "inbox") return 0;
  if (lower === "drafts" || lower.includes("draft")) return 1;
  if (lower === "sent" || lower.includes("sent")) return 2;
  if (lower === "junk" || lower === "spam" || lower.includes("junk") || lower.includes("spam"))
    return 3;
  if (lower === "trash" || lower.includes("trash")) return 4;
  if (lower === "archive" || lower.includes("archive")) return 5;
  return 6; // everything else
}

function getFolderIcon(name: string) {
  const lower = name.toLowerCase();

  if (lower === "inbox") return <Inbox className="size-4" />;
  if (lower === "sent" || lower.includes("sent")) return <Send className="size-4" />;
  if (lower === "drafts" || lower.includes("draft")) return <FileText className="size-4" />;
  if (lower === "trash" || lower.includes("trash")) return <Trash2 className="size-4" />;
  if (lower === "junk" || lower === "spam" || lower.includes("junk") || lower.includes("spam"))
    return <AlertCircle className="size-4" />;
  if (lower === "starred" || lower === "flagged") return <Star className="size-4" />;

  return <Folder className="size-4" />;
}

function SkeletonList() {
  return (
    <div className="flex flex-col gap-1 p-2">
      {Array.from({ length: 5 }).map((_, i) => (
        <div
          key={i}
          className="h-9 animate-pulse rounded-md bg-sidebar-accent"
        />
      ))}
    </div>
  );
}

function InlineRenameInput({
  currentName,
  onDone,
}: {
  currentName: string;
  onDone: () => void;
}) {
  const [value, setValue] = useState(currentName);
  const inputRef = useRef<HTMLInputElement>(null);
  const renameFolder = useRenameFolder();

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  const handleSubmit = useCallback(() => {
    const trimmed = value.trim();
    if (!trimmed || trimmed === currentName) {
      onDone();
      return;
    }
    renameFolder.mutate(
      { name: currentName, newName: trimmed },
      {
        onSuccess: () => onDone(),
        onError: () => {
          // Keep input open on error so user can retry
        },
      },
    );
  }, [value, currentName, renameFolder, onDone]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleSubmit();
      } else if (e.key === "Escape") {
        onDone();
      }
    },
    [handleSubmit, onDone],
  );

  return (
    <div className="flex w-full items-center gap-1 px-3 py-1">
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={handleSubmit}
        className={cn(
          "flex-1 rounded border border-input bg-background px-2 py-1 text-sm text-foreground",
          "outline-none focus:border-ring focus:ring-1 focus:ring-ring/50",
        )}
        disabled={renameFolder.isPending}
        autoComplete="off"
        spellCheck={false}
      />
      {renameFolder.isPending ? (
        <Loader2 className="size-3.5 shrink-0 animate-spin text-muted-foreground" />
      ) : (
        <>
          <button
            type="button"
            onClick={handleSubmit}
            className="rounded p-0.5 text-muted-foreground hover:text-foreground"
            aria-label="Confirm rename"
          >
            <Check className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={onDone}
            className="rounded p-0.5 text-muted-foreground hover:text-foreground"
            aria-label="Cancel rename"
          >
            <X className="size-3.5" />
          </button>
        </>
      )}
      {renameFolder.isError && (
        <span className="text-xs text-destructive" title={renameFolder.error?.message}>
          Error
        </span>
      )}
    </div>
  );
}

function FolderItem({
  folder,
  isRenaming,
  onStartRename,
  onEndRename,
}: {
  folder: FolderType;
  isRenaming: boolean;
  onStartRename: () => void;
  onEndRename: () => void;
}) {
  const activeFolder = useUiStore((s) => s.activeFolder);
  const setActiveFolder = useUiStore((s) => s.setActiveFolder);
  const isActive = activeFolder === folder.name;
  const isFetching = useIsFetching({ queryKey: ["messages", folder.name] });
  const moveMessage = useMoveMessage();
  const [isDragOver, setIsDragOver] = useState(false);
  const dragCounter = useRef(0);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
  }, []);

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    dragCounter.current += 1;
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback(() => {
    dragCounter.current -= 1;
    if (dragCounter.current <= 0) {
      dragCounter.current = 0;
      setIsDragOver(false);
    }
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      dragCounter.current = 0;
      setIsDragOver(false);
      try {
        const raw = e.dataTransfer.getData("application/json");
        if (!raw) return;
        const data = JSON.parse(raw) as {
          uid: number;
          folder: string;
          subject: string;
        };
        if (data.folder === folder.name) return; // same folder, no-op
        moveMessage.mutate({
          fromFolder: data.folder,
          toFolder: folder.name,
          uid: data.uid,
        });
      } catch {
        // ignore malformed drag data
      }
    },
    [folder.name, moveMessage],
  );

  if (isRenaming) {
    return (
      <div
        className={cn(
          "flex w-full items-center gap-3 rounded-md text-sm",
          "bg-sidebar-accent",
        )}
      >
        {getFolderIcon(folder.name)}
        <InlineRenameInput currentName={folder.name} onDone={onEndRename} />
      </div>
    );
  }

  return (
    <FolderContextMenu
      folderName={folder.name}
      onRename={onStartRename}
      onDragOver={handleDragOver}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      <button
        onClick={() => setActiveFolder(folder.name)}
        aria-current={isActive ? "page" : undefined}
        className={cn(
          "flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
          isDragOver
            ? "bg-primary/10 font-semibold text-primary"
            : isActive
              ? "bg-primary/10 font-semibold text-primary"
              : "font-medium text-sidebar-foreground hover:bg-sidebar-accent",
        )}
      >
        {getFolderIcon(folder.name)}
        <span className="flex-1 truncate text-left">{formatFolderName(folder.name)}</span>
        {folder.unread_count > 0 ? (
          <span className="min-w-[20px] rounded-full bg-primary px-1.5 py-0.5 text-center text-xs font-semibold text-primary-foreground">
            {folder.unread_count}
          </span>
        ) : isFetching > 0 ? (
          <Loader2 className="size-3.5 shrink-0 animate-spin text-muted-foreground" />
        ) : null}
      </button>
    </FolderContextMenu>
  );
}

export function FolderTree() {
  const { data, isLoading, isError, refetch } = useFolders();
  const email = useAuthStore((s) => s.email);
  const activeFolder = useUiStore((s) => s.activeFolder);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [renamingFolder, setRenamingFolder] = useState<string | null>(null);

  // Prefetch messages for all folders in the background after folder list loads.
  const folderNames = data?.folders.map((f) => f.name) ?? [];
  usePrefetchAllFolders(folderNames, activeFolder);

  return (
    <div className="flex h-full flex-col">
      {/* User email */}
      <div className="flex items-center justify-center border-b border-sidebar-border px-3 py-3">
        <span className="truncate text-sm font-bold text-sidebar-foreground">
          {email ?? ""}
        </span>
      </div>

      {/* Folder list */}
      <nav className="flex-1 overflow-y-auto">
        {isLoading && <SkeletonList />}

        {isError && (
          <div className="flex flex-col items-center gap-3 px-4 py-8 text-center">
            <p className="text-sm text-muted-foreground">
              Failed to load folders
            </p>
            <Button variant="outline" size="sm" onClick={() => refetch()}>
              Retry
            </Button>
          </div>
        )}

        {data && (
          <div className="flex flex-col gap-0.5">
            {[...data.folders]
              .sort(
                (a, b) =>
                  folderSortOrder(a.name) - folderSortOrder(b.name) ||
                  a.name.localeCompare(b.name),
              )
              .map((folder) => (
                <FolderItem
                  key={folder.name}
                  folder={folder}
                  isRenaming={renamingFolder === folder.name}
                  onStartRename={() => setRenamingFolder(folder.name)}
                  onEndRename={() => setRenamingFolder(null)}
                />
              ))}
          </div>
        )}
      </nav>

      {/* New folder button */}
      <div className="border-t border-sidebar-border p-2">
        <Button
          variant="ghost"
          size="sm"
          className="w-full justify-start gap-2 text-muted-foreground hover:text-foreground"
          onClick={() => setCreateDialogOpen(true)}
        >
          <FolderPlus className="size-4" />
          New folder
        </Button>
      </div>

      {/* Create folder dialog */}
      <CreateFolderDialog
        open={createDialogOpen}
        onClose={() => setCreateDialogOpen(false)}
      />
    </div>
  );
}
