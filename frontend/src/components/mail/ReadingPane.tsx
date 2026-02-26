"use client";

import { useState, useEffect, useCallback } from "react";
import { Dialog, Popover } from "radix-ui";
import {
  Paperclip,
  ChevronDown,
  ChevronUp,
  Code,
  Type,
  FileCode,
  ShieldAlert,
  X,
  Download,
  ChevronLeft,
  ChevronRight,
  UserPlus,
  Send,
  FileText,
  File,
  Copy,
  Check,
} from "lucide-react";
import { useUiStore } from "@/stores/useUiStore";
import {
  useMessage,
  useUpdateFlags,
} from "@/hooks/useMessages";
import { EmailRenderer, hasRemoteResources } from "./EmailRenderer";
import { ThreadView } from "./ThreadView";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useComposeStore } from "@/stores/useComposeStore";
import { useCreateContact } from "@/hooks/useContacts";
import type { EmailAddress, Attachment } from "@/types/message";

type HeaderMode = "summary" | "details";
type BodyMode = "html" | "plain";

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
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

  // Check if yesterday
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (
    date.getFullYear() === yesterday.getFullYear() &&
    date.getMonth() === yesterday.getMonth() &&
    date.getDate() === yesterday.getDate()
  ) {
    return "yesterday";
  }

  // Older — show normal date
  return date.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });
}


function AddressChip({ address, name }: { address: string; name?: string | null }) {
  const displayName = name || address;
  const createContact = useCreateContact();
  const [contactAdded, setContactAdded] = useState(false);

  return (
    <Popover.Root onOpenChange={(open) => { if (!open) setContactAdded(false); }}>
      <Popover.Trigger asChild>
        <button className="inline rounded px-0.5 text-sm text-foreground underline decoration-muted-foreground/30 underline-offset-2 hover:bg-accent hover:decoration-foreground">
          {displayName}
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          className="z-50 w-56 rounded-lg border border-border bg-background p-1 shadow-lg"
          sideOffset={4}
          align="start"
        >
          <div className="border-b border-border px-3 py-2">
            {name && <p className="text-sm font-medium truncate">{name}</p>}
            <p className="text-xs text-muted-foreground truncate">{address}</p>
          </div>
          <button
            onClick={() => {
              useComposeStore.getState().openCompose();
              useComposeStore.setState({ to: address });
            }}
            className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm hover:bg-accent"
          >
            <Send className="size-3.5 text-muted-foreground" />
            Compose email to
          </button>
          <button
            onClick={() => {
              navigator.clipboard.writeText(
                name ? `${name} <${address}>` : address,
              );
            }}
            className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm hover:bg-accent"
          >
            <Copy className="size-3.5 text-muted-foreground" />
            Copy address
          </button>
          <button
            disabled={contactAdded || createContact.isPending}
            onClick={() => {
              createContact.mutate(
                { email: address, name: name ?? "" },
                { onSuccess: () => setContactAdded(true) },
              );
            }}
            className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm hover:bg-accent disabled:opacity-50"
          >
            {contactAdded ? (
              <>
                <Check className="size-3.5 text-green-500" />
                Contact added
              </>
            ) : (
              <>
                <UserPlus className="size-3.5 text-muted-foreground" />
                Add to contacts
              </>
            )}
          </button>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

function AddressList({ addresses }: { addresses: EmailAddress[] }) {
  return (
    <span className="inline">
      {addresses.map((a, i) => (
        <span key={`${a.address}-${i}`}>
          {i > 0 && ", "}
          <AddressChip address={a.address} name={a.name} />
        </span>
      ))}
    </span>
  );
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

function AttachmentPreviewer({
  attachments,
  baseUrl,
  initialIndex,
  onClose,
}: {
  attachments: Attachment[];
  baseUrl: string;
  initialIndex: number;
  onClose: () => void;
}) {
  const [index, setIndex] = useState(initialIndex);
  const att = attachments[index];
  const url = `${baseUrl}/${att.id}`;

  const goPrev = useCallback(() => setIndex((i) => Math.max(0, i - 1)), []);
  const goNext = useCallback(
    () => setIndex((i) => Math.min(attachments.length - 1, i + 1)),
    [attachments.length],
  );

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowLeft") goPrev();
      else if (e.key === "ArrowRight") goNext();
      else if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [goPrev, goNext, onClose]);

  return (
    <Dialog.Root open onOpenChange={(open) => !open && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/70" />
        <Dialog.Content className="fixed inset-4 z-50 flex flex-col rounded-xl border border-border bg-background shadow-2xl">
          {/* Header */}
          <div className="flex items-center justify-between border-b border-border px-4 py-3">
            <Dialog.Title className="flex items-center gap-2 text-sm font-semibold">
              <Paperclip className="size-4 text-muted-foreground" />
              <span className="max-w-[400px] truncate">
                {att.filename ?? "Attachment"}
              </span>
              <span className="text-xs font-normal text-muted-foreground">
                ({formatFileSize(att.size)})
              </span>
              {attachments.length > 1 && (
                <span className="text-xs font-normal text-muted-foreground">
                  — {index + 1} of {attachments.length}
                </span>
              )}
            </Dialog.Title>
            <div className="flex items-center gap-1">
              {attachments.length > 1 && (
                <>
                  <button
                    onClick={goPrev}
                    disabled={index === 0}
                    className="rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-30"
                    title="Previous"
                  >
                    <ChevronLeft className="size-4" />
                  </button>
                  <button
                    onClick={goNext}
                    disabled={index === attachments.length - 1}
                    className="rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-30"
                    title="Next"
                  >
                    <ChevronRight className="size-4" />
                  </button>
                </>
              )}
              <a
                href={url}
                download={att.filename ?? undefined}
                className="rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
                title="Download"
              >
                <Download className="size-4" />
              </a>
              <Dialog.Close asChild>
                <button
                  className="rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
                  title="Close"
                >
                  <X className="size-4" />
                </button>
              </Dialog.Close>
            </div>
          </div>

          {/* Thumbnail strip */}
          {attachments.length > 1 && (
            <div className="flex shrink-0 gap-2 overflow-x-auto border-b border-border bg-muted/30 px-4 py-2">
              {attachments.map((thumb, i) => {
                const thumbUrl = `${baseUrl}/${thumb.id}`;
                const isActive = i === index;
                return (
                  <button
                    key={thumb.id}
                    onClick={() => setIndex(i)}
                    className={cn(
                      "flex size-14 shrink-0 items-center justify-center overflow-hidden rounded-md border-2 transition-colors",
                      isActive
                        ? "border-primary bg-accent"
                        : "border-transparent bg-muted hover:border-muted-foreground/30",
                    )}
                    title={thumb.filename ?? `Attachment ${i + 1}`}
                  >
                    {thumb.content_type.startsWith("image/") ? (
                      <img
                        src={thumbUrl}
                        alt={thumb.filename ?? ""}
                        className="size-full object-cover"
                      />
                    ) : thumb.content_type === "application/pdf" ? (
                      <FileText className="size-6 text-muted-foreground" />
                    ) : (
                      <File className="size-6 text-muted-foreground" />
                    )}
                  </button>
                );
              })}
            </div>
          )}

          {/* Preview content */}
          <div className="flex flex-1 items-center justify-center overflow-auto p-4">
            {att.content_type.startsWith("image/") ? (
              <img
                src={url}
                alt={att.filename ?? "Attachment"}
                className="max-h-full max-w-full object-contain"
              />
            ) : att.content_type === "application/pdf" ? (
              <iframe
                src={url}
                className="h-full w-full border-none"
                title={att.filename ?? "PDF"}
              />
            ) : (
              <div className="flex flex-col items-center gap-4 text-center">
                <Paperclip className="size-12 text-muted-foreground" />
                <p className="text-sm text-muted-foreground">
                  Preview not available for this file type
                </p>
                <a
                  href={url}
                  download={att.filename ?? undefined}
                  className="inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
                >
                  <Download className="size-4" />
                  Download
                </a>
              </div>
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function ReadingPane() {
  const activeFolder = useUiStore((s) => s.activeFolder);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const [headerMode, setHeaderMode] = useState<HeaderMode>("details");
  const [bodyMode, setBodyMode] = useState<BodyMode>("html");
  const [showHeaders, setShowHeaders] = useState(false);
  const [allowedRemoteUids, setAllowedRemoteUids] = useState<Set<string>>(new Set());
  const [previewIndex, setPreviewIndex] = useState<number | null>(null);

  const { data, isLoading, isError, refetch } = useMessage(
    activeFolder,
    selectedMessageUid ?? 0,
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
            From{" "}
            <AddressChip address={data.from_address} name={data.from_name || null} />
            {" "}on {humanizeDate(data.date)}
          </div>
        ) : (
          <>
            <div className="text-sm text-foreground">
              <span className="font-medium text-muted-foreground">From: </span>
              <AddressChip address={data.from_address} name={data.from_name || null} />
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
