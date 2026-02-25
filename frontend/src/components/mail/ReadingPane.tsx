"use client";

import {
  Paperclip,
  Star,
  Mail,
  MailOpen,
  Trash2,
} from "lucide-react";
import { useUiStore } from "@/stores/useUiStore";
import {
  useMessage,
  useUpdateFlags,
  useMoveMessage,
  useDeleteMessage,
} from "@/hooks/useMessages";
import { EmailRenderer } from "./EmailRenderer";
import { ThreadView } from "./ThreadView";
import { MoveToFolderMenu } from "./MoveToFolderMenu";
import { Button } from "@/components/ui/button";
import type { EmailAddress } from "@/types/message";

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function formatDate(iso: string): string {
  const date = new Date(iso);
  return date.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });
}

function formatAddressList(addresses: EmailAddress[]): string {
  return addresses
    .map((a) => (a.name ? `${a.name} <${a.address}>` : a.address))
    .join(", ");
}

function HeaderSkeleton() {
  return (
    <div className="space-y-3 border-b border-border p-4">
      <div className="h-6 w-3/4 animate-pulse rounded bg-muted" />
      <div className="h-4 w-1/2 animate-pulse rounded bg-muted" />
      <div className="h-4 w-1/3 animate-pulse rounded bg-muted" />
      <div className="h-4 w-1/4 animate-pulse rounded bg-muted" />
    </div>
  );
}

function BodySkeleton() {
  return (
    <div className="space-y-2 p-4">
      <div className="h-4 w-full animate-pulse rounded bg-muted" />
      <div className="h-4 w-full animate-pulse rounded bg-muted" />
      <div className="h-4 w-5/6 animate-pulse rounded bg-muted" />
      <div className="h-4 w-full animate-pulse rounded bg-muted" />
      <div className="h-4 w-2/3 animate-pulse rounded bg-muted" />
    </div>
  );
}

export function ReadingPane() {
  const activeFolder = useUiStore((s) => s.activeFolder);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const selectMessage = useUiStore((s) => s.selectMessage);

  const { data, isLoading, isError, refetch } = useMessage(
    activeFolder,
    selectedMessageUid ?? 0,
  );

  const updateFlags = useUpdateFlags();
  const moveMessage = useMoveMessage();
  const deleteMessage = useDeleteMessage();

  // No message selected
  if (selectedMessageUid === null) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        Select a message to read
      </div>
    );
  }

  // Loading
  if (isLoading) {
    return (
      <div className="flex h-full flex-col overflow-y-auto">
        <HeaderSkeleton />
        <BodySkeleton />
      </div>
    );
  }

  // Error
  if (isError || !data) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-4 py-8 text-center">
        <p className="text-sm text-muted-foreground">
          Failed to load message
        </p>
        <Button variant="outline" size="sm" onClick={() => refetch()}>
          Retry
        </Button>
      </div>
    );
  }

  const attachmentBaseUrl = `/api/messages/${encodeURIComponent(data.folder)}/${data.uid}/attachments`;

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      {/* Header area */}
      <div className="shrink-0 space-y-1 border-b border-border p-4">
        <h2 className="text-lg font-bold leading-tight">{data.subject}</h2>

        <div className="text-sm text-foreground">
          <span className="font-medium text-muted-foreground">From: </span>
          {data.from_name ? (
            <>
              {data.from_name}{" "}
              <span className="text-muted-foreground">
                &lt;{data.from_address}&gt;
              </span>
            </>
          ) : (
            data.from_address
          )}
        </div>

        <div className="text-sm text-foreground">
          <span className="font-medium text-muted-foreground">To: </span>
          {formatAddressList(data.to_addresses)}
        </div>

        {data.cc_addresses.length > 0 && (
          <div className="text-sm text-foreground">
            <span className="font-medium text-muted-foreground">Cc: </span>
            {formatAddressList(data.cc_addresses)}
          </div>
        )}

        <div className="text-sm text-muted-foreground">
          {formatDate(data.date)}
        </div>
      </div>

      {/* Toolbar */}
      <div className="flex shrink-0 items-center gap-1 border-b border-border px-4 py-1.5">
        {/* Read/Unread toggle */}
        <Button
          variant="ghost"
          size="sm"
          className="gap-1.5"
          onClick={() => {
            const isSeen = data.flags.includes("\\Seen");
            updateFlags.mutate({
              folder: activeFolder,
              uid: data.uid,
              flags: ["\\Seen"],
              add: !isSeen,
            });
          }}
        >
          {data.flags.includes("\\Seen") ? (
            <>
              <Mail className="size-4" />
              Mark unread
            </>
          ) : (
            <>
              <MailOpen className="size-4" />
              Mark read
            </>
          )}
        </Button>

        {/* Star/Unstar toggle */}
        <Button
          variant="ghost"
          size="sm"
          className="gap-1.5"
          onClick={() => {
            const isFlagged = data.flags.includes("\\Flagged");
            updateFlags.mutate({
              folder: activeFolder,
              uid: data.uid,
              flags: ["\\Flagged"],
              add: !isFlagged,
            });
          }}
        >
          {data.flags.includes("\\Flagged") ? (
            <>
              <Star className="size-4 fill-primary text-primary" />
              Unstar
            </>
          ) : (
            <>
              <Star className="size-4" />
              Star
            </>
          )}
        </Button>

        <div className="mx-1 h-5 w-px bg-border" />

        {/* Delete button */}
        <Button
          variant="ghost"
          size="sm"
          className="gap-1.5"
          onClick={() => {
            if (activeFolder === "Trash") {
              deleteMessage.mutate(
                { folder: activeFolder, uid: data.uid },
                { onSuccess: () => selectMessage(null) },
              );
            } else {
              moveMessage.mutate(
                {
                  fromFolder: activeFolder,
                  toFolder: "Trash",
                  uid: data.uid,
                },
                { onSuccess: () => selectMessage(null) },
              );
            }
          }}
        >
          <Trash2 className="size-4" />
          {activeFolder === "Trash" ? "Delete forever" : "Delete"}
        </Button>

        {/* Move to folder */}
        <MoveToFolderMenu
          currentFolder={activeFolder}
          onMove={(toFolder) => {
            moveMessage.mutate(
              {
                fromFolder: activeFolder,
                toFolder,
                uid: data.uid,
              },
              { onSuccess: () => selectMessage(null) },
            );
          }}
        />
      </div>

      {/* Attachment bar */}
      {data.attachments.length > 0 && (
        <div className="flex shrink-0 gap-2 overflow-x-auto border-b border-border px-4 py-2">
          {data.attachments.map((att) => (
            <a
              key={att.id}
              href={`${attachmentBaseUrl}/${att.id}`}
              download={att.filename ?? undefined}
              className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-border bg-muted/50 px-2.5 py-1 text-xs text-foreground transition-colors hover:bg-muted"
            >
              <Paperclip className="size-3.5 shrink-0 text-muted-foreground" />
              <span className="max-w-[200px] truncate">
                {att.filename ?? "Attachment"}
              </span>
              <span className="text-muted-foreground">
                ({formatFileSize(att.size)})
              </span>
            </a>
          ))}
        </div>
      )}

      {/* Thread view — only shown when there are multiple messages in the thread */}
      {data.thread && data.thread.length > 1 && (
        <ThreadView thread={data.thread} currentUid={data.uid} />
      )}

      {/* Body area */}
      <div className="min-h-0 flex-1">
        <EmailRenderer html={data.html} text={data.text} />
      </div>
    </div>
  );
}
