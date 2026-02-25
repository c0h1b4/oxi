"use client";

import { useEffect, useState } from "react";
import {
  Inbox,
  Send,
  FileText,
  Trash2,
  AlertCircle,
  Star,
  Folder,
  Loader2,
  PenLine,
  X,
} from "lucide-react";
import { useIsFetching } from "@tanstack/react-query";
import { useFolders } from "@/hooks/useFolders";
import { useListDrafts, useGetDraft, useDeleteDraft } from "@/hooks/useCompose";
import { usePrefetchAllFolders } from "@/hooks/useMessages";
import { useUiStore } from "@/stores/useUiStore";
import { useAuthStore } from "@/stores/useAuthStore";
import { useComposeStore } from "@/stores/useComposeStore";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { Folder as FolderType } from "@/types/folder";

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
      <span className="flex-1 truncate text-left">{folder.name}</span>
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

function LocalDrafts() {
  const { data } = useListDrafts(true);
  const deleteDraft = useDeleteDraft();
  const openDraft = useComposeStore((s) => s.openDraft);
  const isComposeOpen = useComposeStore((s) => s.isOpen);
  const [loadingDraftId, setLoadingDraftId] = useState<string | null>(null);
  const getDraft = useGetDraft(loadingDraftId);

  // When draft detail loads, open compose dialog
  useEffect(() => {
    if (getDraft.data && loadingDraftId) {
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
      setLoadingDraftId(null);
    }
  }, [getDraft.data, loadingDraftId, openDraft]);

  const drafts = data?.drafts ?? [];
  if (drafts.length === 0) return null;

  return (
    <div className="border-t border-sidebar-border pt-2">
      <div className="px-3 pb-1 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
        Local Drafts
      </div>
      <div className="flex flex-col gap-0.5">
        {drafts.map((draft) => (
          <div
            key={draft.id}
            className="group flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-sidebar-foreground hover:bg-sidebar-accent"
          >
            <PenLine className="size-4 shrink-0 text-muted-foreground" />
            <button
              onClick={() => {
                if (!isComposeOpen) {
                  setLoadingDraftId(draft.id);
                }
              }}
              className="flex-1 truncate text-left"
              title={draft.subject || "(No subject)"}
            >
              {draft.subject || "(No subject)"}
            </button>
            <button
              onClick={(e) => {
                e.stopPropagation();
                deleteDraft.mutate(draft.id);
              }}
              className="hidden shrink-0 rounded p-0.5 text-muted-foreground hover:bg-accent hover:text-foreground group-hover:block"
              title="Delete draft"
            >
              <X className="size-3" />
            </button>
          </div>
        ))}
      </div>
    </div>
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

        <LocalDrafts />
      </nav>
    </div>
  );
}
