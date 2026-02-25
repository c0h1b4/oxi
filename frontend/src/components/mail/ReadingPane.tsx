"use client";

import { useState, useEffect } from "react";
import {
  Paperclip,
  Star,
  Mail,
  MailOpen,
  Trash2,
  ChevronDown,
  ChevronUp,
  Code,
  Type,
  FileCode,
  ShieldAlert,
} from "lucide-react";
import { useUiStore } from "@/stores/useUiStore";
import {
  useMessage,
  useUpdateFlags,
  useMoveMessage,
  useDeleteMessage,
} from "@/hooks/useMessages";
import { EmailRenderer, hasRemoteResources } from "./EmailRenderer";
import { ThreadView } from "./ThreadView";
import { MoveToFolderMenu } from "./MoveToFolderMenu";
import { Button } from "@/components/ui/button";
import type { EmailAddress } from "@/types/message";

type HeaderMode = "summary" | "details";
type BodyMode = "html" | "plain";

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
  const [headerMode, setHeaderMode] = useState<HeaderMode>("details");
  const [bodyMode, setBodyMode] = useState<BodyMode>("html");
  const [showHeaders, setShowHeaders] = useState(false);
  const [allowedRemoteUids, setAllowedRemoteUids] = useState<Set<string>>(new Set());

  const { data, isLoading, isError, refetch } = useMessage(
    activeFolder,
    selectedMessageUid ?? 0,
  );

  const updateFlags = useUpdateFlags();
  const moveMessage = useMoveMessage();
  const deleteMessage = useDeleteMessage();

  // Auto-mark unread messages as read when opened.
  useEffect(() => {
    if (data && !data.flags.includes("\\Seen")) {
      updateFlags.mutate({
        folder: activeFolder,
        uid: data.uid,
        flags: ["\\Seen"],
        add: true,
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data?.uid, data?.folder]);

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
  const messageKey = `${data.folder}:${data.uid}`;
  const remoteAllowed = allowedRemoteUids.has(messageKey);
  const showRemoteBanner = !remoteAllowed && hasRemoteResources(data.html);

  return (
    <div className="flex h-full w-full flex-col overflow-hidden">
      {/* Header area */}
      <div className="shrink-0 space-y-1 overflow-x-hidden border-b border-border p-4">
        <h2 className="text-lg font-bold leading-tight">{data.subject}</h2>

        {headerMode === "summary" ? (
          <div className="text-sm text-foreground">
            From {data.from_address} on {formatDate(data.date)}
          </div>
        ) : (
          <>
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
          </>
        )}

        {/* View toggle buttons */}
        <div className="flex gap-1 pt-1">
          {/* Details / Summary toggle */}
          <button
            onClick={() => setHeaderMode(headerMode === "details" ? "summary" : "details")}
            className="inline-flex items-center gap-1 rounded px-2 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            {headerMode === "details" ? (
              <ChevronUp className="size-3" />
            ) : (
              <ChevronDown className="size-3" />
            )}
            {headerMode === "details" ? "Summary" : "Details"}
          </button>

          {/* Plain text / HTML toggle */}
          <button
            onClick={() => {
              setBodyMode(bodyMode === "html" ? "plain" : "html");
              setShowHeaders(false);
            }}
            className="inline-flex items-center gap-1 rounded px-2 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            {bodyMode === "html" ? (
              <Type className="size-3" />
            ) : (
              <Code className="size-3" />
            )}
            {bodyMode === "html" ? "Plain text" : "HTML"}
          </button>

          {/* Headers toggle (shows as selected when active) */}
          <button
            onClick={() => setShowHeaders(!showHeaders)}
            className={`inline-flex items-center gap-1 rounded px-2 py-0.5 text-xs transition-colors ${
              showHeaders
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-muted hover:text-foreground"
            }`}
          >
            <FileCode className="size-3" />
            Headers
          </button>
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

      {/* Remote resources banner */}
      {showRemoteBanner && (
        <div className="flex shrink-0 items-center gap-2 border-b border-border bg-muted/50 px-4 py-2">
          <ShieldAlert className="size-4 shrink-0 text-muted-foreground" />
          <span className="flex-1 text-xs text-muted-foreground">
            To protect your privacy, remote resources have been blocked.
          </span>
          <Button
            variant="outline"
            size="sm"
            className="h-6 text-xs"
            onClick={() =>
              setAllowedRemoteUids((prev) => new Set(prev).add(messageKey))
            }
          >
            Allow
          </Button>
        </div>
      )}

      {/* Body area — fills remaining space */}
      <div className="min-h-0 flex-1">
        {showHeaders ? (
          <pre className="h-full overflow-auto whitespace-pre-wrap break-words p-4 text-xs leading-relaxed text-foreground">
            {data.raw_headers || "No headers available"}
          </pre>
        ) : bodyMode === "plain" ? (
          <pre className="h-full overflow-auto whitespace-pre-wrap break-words p-4 text-sm leading-relaxed text-foreground">
            {data.text || "No plain text available"}
          </pre>
        ) : (
          <EmailRenderer
            html={data.html}
            text={data.text}
            blockRemoteResources={!remoteAllowed}
          />
        )}
      </div>
    </div>
  );
}
