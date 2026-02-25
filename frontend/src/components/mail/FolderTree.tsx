"use client";

import {
  Inbox,
  Send,
  FileText,
  Trash2,
  AlertCircle,
  Star,
  Folder,
} from "lucide-react";
import { useFolders } from "@/hooks/useFolders";
import { useUiStore } from "@/stores/useUiStore";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { Folder as FolderType } from "@/types/folder";

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

  return (
    <button
      onClick={() => setActiveFolder(folder.name)}
      className={cn(
        "flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-sidebar-foreground transition-colors hover:bg-sidebar-accent",
        isActive && "bg-sidebar-accent text-sidebar-accent-foreground",
      )}
    >
      {getFolderIcon(folder.name)}
      <span className="flex-1 truncate text-left">{folder.name}</span>
      {folder.unread_count > 0 && (
        <span className="min-w-[20px] rounded-full bg-primary px-1.5 py-0.5 text-center text-xs font-semibold text-primary-foreground">
          {folder.unread_count}
        </span>
      )}
    </button>
  );
}

export function FolderTree() {
  const { data, isLoading, isError, refetch } = useFolders();

  return (
    <div className="flex h-full flex-col">
      {/* Branding */}
      <div className="flex items-center px-4 py-4">
        <h1 className="text-lg font-bold tracking-tight text-sidebar-foreground">
          oxi<span className="text-primary">.email</span>
        </h1>
      </div>

      {/* Folder list */}
      <nav className="flex-1 overflow-y-auto px-2">
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
            {data.folders.map((folder) => (
              <FolderItem key={folder.name} folder={folder} />
            ))}
          </div>
        )}
      </nav>
    </div>
  );
}
