"use client";

import {
  Inbox,
  Send,
  FileText,
  Trash2,
  AlertCircle,
  Star,
  Folder,
  Loader2,
} from "lucide-react";
import { useIsFetching } from "@tanstack/react-query";
import { useFolders } from "@/hooks/useFolders";
import { usePrefetchAllFolders } from "@/hooks/useMessages";
import { useUiStore } from "@/stores/useUiStore";
import { useAuthStore } from "@/stores/useAuthStore";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { Folder as FolderType } from "@/types/folder";

/** Check if a folder name refers to the Drafts folder. */
export function isDraftsFolder(name: string): boolean {
  const lower = name.toLowerCase();
  return lower === "drafts" || lower.includes("draft");
}

/** Title case: first letter uppercase, rest lowercase. E.g. "INBOX" → "Inbox" */
export function formatFolderName(name: string): string {
  if (!name) return name;
  return name.charAt(0).toUpperCase() + name.slice(1).toLowerCase();
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

function FolderItem({ folder }: { folder: FolderType }) {
  const activeFolder = useUiStore((s) => s.activeFolder);
  const setActiveFolder = useUiStore((s) => s.setActiveFolder);
  const isActive = activeFolder === folder.name;
  const isFetching = useIsFetching({ queryKey: ["messages", folder.name] });

  return (
    <button
      onClick={() => setActiveFolder(folder.name)}
      aria-current={isActive ? "page" : undefined}
      className={cn(
        "flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
        isActive
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
  );
}

export function FolderTree() {
  const { data, isLoading, isError, refetch } = useFolders();
  const email = useAuthStore((s) => s.email);
  const activeFolder = useUiStore((s) => s.activeFolder);

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
                <FolderItem key={folder.name} folder={folder} />
              ))}
          </div>
        )}

      </nav>
    </div>
  );
}
