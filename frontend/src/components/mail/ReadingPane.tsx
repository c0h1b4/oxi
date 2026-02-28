"use client";

import { useState, useEffect } from "react";
import {
  Paperclip,
  ChevronDown,
  ChevronUp,
  Code,
  Type,
  FileCode,
  ShieldAlert,
} from "lucide-react";
import { useUiStore } from "@/stores/useUiStore";
import { useMessage, useUpdateFlags } from "@/hooks/useMessages";
import { EmailRenderer, hasRemoteResources } from "./EmailRenderer";
import { ThreadView } from "./ThreadView";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import {
  AddressChip,
  AddressList,
  AttachmentPreviewer,
  HeaderSkeleton,
  BodySkeleton,
  formatFileSize,
  humanizeDate,
} from "./ReadingPane/index";

type HeaderMode = "summary" | "details";
type BodyMode = "html" | "plain";

export function ReadingPane() {
  const activeFolder = useUiStore((s) => s.activeFolder);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const [headerMode, setHeaderMode] = useState<HeaderMode>("details");
  const [bodyMode, setBodyMode] = useState<BodyMode>("html");
  const [showHeaders, setShowHeaders] = useState(false);
  const [allowedRemoteUids, setAllowedRemoteUids] = useState<Set<string>>(
    new Set()
  );
  const [previewIndex, setPreviewIndex] = useState<number | null>(null);

  const { data, isLoading, isError, isPlaceholderData, refetch } = useMessage(
    activeFolder,
    selectedMessageUid ?? 0
  );

  const updateFlags = useUpdateFlags();

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
        <p className="text-sm text-muted-foreground">Failed to load message</p>
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
    <div
      className={cn(
        "flex h-full w-full flex-col overflow-hidden transition-opacity",
        isPlaceholderData && "opacity-40 pointer-events-none"
      )}
    >
      {/* Header area */}
      <div className="shrink-0 space-y-1 overflow-x-hidden border-b border-border p-4">
        <h2 className="text-lg font-bold leading-tight">{data.subject}</h2>

        {headerMode === "summary" ? (
          <div className="text-sm text-foreground">
            From{" "}
            <AddressChip
              address={data.from_address}
              name={data.from_name || null}
            />
            {" "}
            on {humanizeDate(data.date)}
          </div>
        ) : (
          <>
            <div className="text-sm text-foreground">
              <span className="font-medium text-muted-foreground">From: </span>
              <AddressChip
                address={data.from_address}
                name={data.from_name || null}
              />
            </div>

            <div className="text-sm text-foreground">
              <span className="font-medium text-muted-foreground">To: </span>
              <AddressList addresses={data.to_addresses} />
            </div>

            {data.cc_addresses.length > 0 && (
              <div className="text-sm text-foreground">
                <span className="font-medium text-muted-foreground">Cc: </span>
                <AddressList addresses={data.cc_addresses} />
              </div>
            )}

            <div className="text-sm text-muted-foreground">
              {humanizeDate(data.date)}
            </div>
          </>
        )}

        {/* View toggle buttons */}
        <div className="flex gap-1 pt-1">
          {/* Details / Summary toggle */}
          <button
            onClick={() =>
              setHeaderMode(headerMode === "details" ? "summary" : "details")
            }
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

      {/* Attachment bar */}
      {data.attachments.length > 0 && (
        <div className="flex shrink-0 gap-2 overflow-x-auto border-b border-border px-4 py-2">
          {data.attachments.map((att, i) => (
            <button
              key={att.id}
              onClick={() => setPreviewIndex(i)}
              className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-border bg-muted/50 px-2.5 py-1 text-xs text-foreground transition-colors hover:bg-muted"
            >
              <Paperclip className="size-3.5 shrink-0 text-muted-foreground" />
              <span className="max-w-[200px] truncate">
                {att.filename ?? "Attachment"}
              </span>
              <span className="text-muted-foreground">
                ({formatFileSize(att.size)})
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Attachment previewer */}
      {previewIndex !== null && (
        <AttachmentPreviewer
          attachments={data.attachments}
          baseUrl={attachmentBaseUrl}
          initialIndex={previewIndex}
          onClose={() => setPreviewIndex(null)}
        />
      )}

      {/* Thread view — only shown when there are multiple messages in the thread */}
      {data.thread && data.thread.length > 1 && (() => {
        const visibleThread = data.thread.filter(
          (m) => m.folder === activeFolder || m.uid === data.uid,
        );
        return visibleThread.length > 1 ? (
          <ThreadView thread={visibleThread} currentUid={data.uid} />
        ) : null;
      })()}

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
