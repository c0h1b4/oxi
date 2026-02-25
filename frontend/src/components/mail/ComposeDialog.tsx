"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Dialog } from "radix-ui";
import {
  Send,
  X,
  ChevronUp,
  AlertTriangle,
  Paperclip,
  Loader2,
} from "lucide-react";
import { useComposeStore } from "@/stores/useComposeStore";
import {
  useSendMessage,
  useUploadAttachment,
  useDeleteAttachment,
} from "@/hooks/useCompose";
import { cn } from "@/lib/utils";

function countRecipients(...fields: string[]): number {
  return fields.reduce(
    (count, field) =>
      count +
      field
        .split(",")
        .map((s) => s.trim())
        .filter((s) => s.length > 0).length,
    0,
  );
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Generate a UUID v4 (crypto-based). */
function generateId(): string {
  return crypto.randomUUID();
}

export function ComposeDialog() {
  const {
    isOpen,
    to,
    cc,
    bcc,
    subject,
    body,
    inReplyTo,
    references,
    draftId,
    showCc,
    showBcc,
    attachments,
    closeCompose,
    setField,
    setShowCc,
    setShowBcc,
    setDraftId,
    addAttachments,
    removeAttachment,
    reset,
  } = useComposeStore();

  const sendMutation = useSendMessage();
  const uploadMutation = useUploadAttachment();
  const deleteMutation = useDeleteAttachment();
  const [undoTimer, setUndoTimer] = useState<ReturnType<
    typeof setTimeout
  > | null>(null);
  const [sending, setSending] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const toInputRef = useRef<HTMLInputElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Auto-focus the To field when dialog opens
  useEffect(() => {
    if (isOpen && toInputRef.current) {
      // Small delay to let the dialog animation complete
      const t = setTimeout(() => toInputRef.current?.focus(), 100);
      return () => clearTimeout(t);
    }
  }, [isOpen]);

  // Clear toast after 5s
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 5000);
    return () => clearTimeout(t);
  }, [toast]);

  const doSend = useCallback(() => {
    sendMutation.mutate(
      { to, cc, bcc, subject, body, inReplyTo, references, draftId },
      {
        onSuccess: () => {
          setToast("Message sent");
          reset();
        },
        onError: (error) => {
          setToast(`Failed to send: ${error.message}`);
          setSending(false);
        },
      },
    );
  }, [
    to,
    cc,
    bcc,
    subject,
    body,
    inReplyTo,
    references,
    draftId,
    sendMutation,
    reset,
  ]);

  const handleSend = useCallback(() => {
    if (!to.trim() && !cc.trim() && !bcc.trim()) return;
    setSending(true);
    closeCompose();

    // 5-second undo window
    const timer = setTimeout(() => {
      setUndoTimer(null);
      doSend();
    }, 5000);
    setUndoTimer(timer);
  }, [to, cc, bcc, closeCompose, doSend]);

  const handleUndo = useCallback(() => {
    if (undoTimer) {
      clearTimeout(undoTimer);
      setUndoTimer(null);
      setSending(false);
      // Re-open the compose dialog with the same content
      useComposeStore.setState({ isOpen: true });
    }
  }, [undoTimer]);

  const handleDiscard = useCallback(() => {
    reset();
  }, [reset]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend],
  );

  const handleAttachFiles = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileSelected = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files;
      if (!files || files.length === 0) return;

      // Ensure we have a draft ID for uploads
      let currentDraftId = draftId;
      if (!currentDraftId) {
        currentDraftId = generateId();
        setDraftId(currentDraftId);
      }

      uploadMutation.mutate(
        { draftId: currentDraftId, files: Array.from(files) },
        {
          onSuccess: (data) => {
            addAttachments(
              data.attachments.map((a) => ({
                id: a.id,
                filename: a.filename,
                contentType: a.content_type,
                size: a.size,
              })),
            );
          },
          onError: (error) => {
            setToast(`Upload failed: ${error.message}`);
          },
        },
      );

      // Reset the input so the same file can be re-selected
      e.target.value = "";
    },
    [draftId, setDraftId, uploadMutation, addAttachments],
  );

  const handleRemoveAttachment = useCallback(
    (attachmentId: string) => {
      if (!draftId) return;
      deleteMutation.mutate(
        { draftId, attachmentId },
        {
          onSuccess: () => {
            removeAttachment(attachmentId);
          },
          onError: (error) => {
            setToast(`Delete failed: ${error.message}`);
          },
        },
      );
    },
    [draftId, deleteMutation, removeAttachment],
  );

  return (
    <>
      <Dialog.Root
        open={isOpen}
        onOpenChange={(open) => {
          if (!open) closeCompose();
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-40 bg-black/40" />
          <Dialog.Content
            className="fixed inset-x-4 bottom-4 top-auto z-50 mx-auto flex max-h-[80vh] w-full max-w-2xl flex-col rounded-xl border border-border bg-background shadow-2xl sm:inset-x-auto sm:bottom-8"
            onKeyDown={handleKeyDown}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-border px-4 py-3">
              <Dialog.Title className="text-sm font-semibold">
                New Message
              </Dialog.Title>
              <Dialog.Close asChild>
                <button
                  className="rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
                  title="Close"
                >
                  <X className="size-4" />
                </button>
              </Dialog.Close>
            </div>

            {/* Fields */}
            <div className="flex flex-col border-b border-border">
              <div className="flex items-center border-b border-border/50 px-4">
                <label className="w-12 shrink-0 text-xs text-muted-foreground">
                  To
                </label>
                <input
                  ref={toInputRef}
                  type="text"
                  value={to}
                  onChange={(e) => setField("to", e.target.value)}
                  placeholder="Recipients"
                  className="flex-1 bg-transparent py-2 text-sm outline-none placeholder:text-muted-foreground/50"
                />
                <button
                  className="ml-2 text-xs text-muted-foreground hover:text-foreground"
                  onClick={() => {
                    if (!showCc && !showBcc) {
                      setShowCc(true);
                    } else {
                      setShowCc(!showCc);
                      setShowBcc(!showBcc);
                    }
                  }}
                >
                  {showCc || showBcc ? (
                    <ChevronUp className="size-3.5" />
                  ) : (
                    <span>Cc Bcc</span>
                  )}
                </button>
              </div>

              {showCc && (
                <div className="flex items-center border-b border-border/50 px-4">
                  <label className="w-12 shrink-0 text-xs text-muted-foreground">
                    Cc
                  </label>
                  <input
                    type="text"
                    value={cc}
                    onChange={(e) => setField("cc", e.target.value)}
                    className="flex-1 bg-transparent py-2 text-sm outline-none placeholder:text-muted-foreground/50"
                  />
                </div>
              )}

              {showBcc && (
                <div className="flex items-center border-b border-border/50 px-4">
                  <label className="w-12 shrink-0 text-xs text-muted-foreground">
                    Bcc
                  </label>
                  <input
                    type="text"
                    value={bcc}
                    onChange={(e) => setField("bcc", e.target.value)}
                    className="flex-1 bg-transparent py-2 text-sm outline-none placeholder:text-muted-foreground/50"
                  />
                </div>
              )}

              <div className="flex items-center px-4">
                <label className="w-12 shrink-0 text-xs text-muted-foreground">
                  Subject
                </label>
                <input
                  type="text"
                  value={subject}
                  onChange={(e) => setField("subject", e.target.value)}
                  className="flex-1 bg-transparent py-2 text-sm outline-none placeholder:text-muted-foreground/50"
                />
              </div>
            </div>

            {/* Recipient count warning */}
            {countRecipients(to, cc, bcc) > 10 && (
              <div className="flex items-center gap-2 border-b border-yellow-300/50 bg-yellow-50 px-4 py-2 dark:border-yellow-700/50 dark:bg-yellow-950/30">
                <AlertTriangle className="size-4 shrink-0 text-yellow-600 dark:text-yellow-500" />
                <span className="text-xs text-yellow-700 dark:text-yellow-400">
                  You are sending to more than 10 recipients.
                </span>
              </div>
            )}

            {/* Attachments */}
            {attachments.length > 0 && (
              <div className="flex flex-wrap gap-2 border-b border-border px-4 py-2">
                {attachments.map((att) => (
                  <div
                    key={att.id}
                    className="flex items-center gap-1.5 rounded-md border border-border bg-accent/50 px-2 py-1 text-xs"
                  >
                    <Paperclip className="size-3 shrink-0 text-muted-foreground" />
                    <span className="max-w-[150px] truncate" title={att.filename}>
                      {att.filename}
                    </span>
                    <span className="text-muted-foreground">
                      ({formatFileSize(att.size)})
                    </span>
                    <button
                      onClick={() => handleRemoveAttachment(att.id)}
                      className="ml-0.5 rounded p-0.5 text-muted-foreground hover:bg-accent hover:text-foreground"
                      title="Remove attachment"
                    >
                      <X className="size-3" />
                    </button>
                  </div>
                ))}
              </div>
            )}

            {/* Body */}
            <div className="flex-1 overflow-auto px-4 py-3">
              <textarea
                value={body}
                onChange={(e) => setField("body", e.target.value)}
                placeholder="Write your message..."
                className="min-h-[200px] w-full resize-none bg-transparent text-sm outline-none placeholder:text-muted-foreground/50"
              />
            </div>

            {/* Footer */}
            <div className="flex items-center justify-between border-t border-border px-4 py-3">
              <div className="flex items-center gap-2">
                <button
                  onClick={handleSend}
                  disabled={
                    sendMutation.isPending ||
                    (!to.trim() && !cc.trim() && !bcc.trim())
                  }
                  className={cn(
                    "inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition-colors",
                    "bg-primary text-primary-foreground hover:bg-primary/90",
                    "disabled:cursor-not-allowed disabled:opacity-50",
                  )}
                >
                  <Send className="size-4" />
                  Send
                </button>
                <button
                  onClick={handleAttachFiles}
                  disabled={uploadMutation.isPending}
                  className="rounded-lg p-2 text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-50"
                  title="Attach files"
                >
                  {uploadMutation.isPending ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <Paperclip className="size-4" />
                  )}
                </button>
                <input
                  ref={fileInputRef}
                  type="file"
                  multiple
                  className="hidden"
                  onChange={handleFileSelected}
                />
              </div>
              <button
                onClick={handleDiscard}
                className="rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
              >
                Discard
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      {/* Undo-send toast */}
      {undoTimer && (
        <div className="fixed bottom-6 left-1/2 z-50 flex -translate-x-1/2 items-center gap-3 rounded-lg bg-foreground px-4 py-2.5 text-sm text-background shadow-lg">
          <span>Sending message...</span>
          <button
            onClick={handleUndo}
            className="font-semibold text-primary underline underline-offset-2"
          >
            Undo
          </button>
        </div>
      )}

      {/* Status toast */}
      {toast && !undoTimer && (
        <div className="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-lg bg-foreground px-4 py-2.5 text-sm text-background shadow-lg">
          {toast}
        </div>
      )}
    </>
  );
}
