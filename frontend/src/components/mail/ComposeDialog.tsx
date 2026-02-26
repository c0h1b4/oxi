"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { AlertDialog, Dialog } from "radix-ui";
import {
  Send,
  X,
  ChevronUp,
  AlertTriangle,
  Paperclip,
  Loader2,
  Save,
  Maximize2,
  Minimize2,
  Download,
  Upload,
} from "lucide-react";
import { toast } from "sonner";
import { useComposeStore } from "@/stores/useComposeStore";
import {
  useSendMessage,
  useSaveDraft,
  useUploadAttachment,
  useDeleteAttachment,
  useDeleteDraft,
} from "@/hooks/useCompose";
import { RichTextEditor } from "@/components/mail/RichTextEditor";
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

/** Strip HTML tags to produce a plain-text fallback. */
function stripHtml(html: string): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  return doc.body.textContent ?? "";
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
  const saveDraftMutation = useSaveDraft();
  const uploadMutation = useUploadAttachment();
  const deleteMutation = useDeleteAttachment();
  const deleteDraftMutation = useDeleteDraft();
  const [sending, setSending] = useState(false);
  const [draftSaved, setDraftSaved] = useState(false);
  const [showDiscardAlert, setShowDiscardAlert] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [previewAttId, setPreviewAttId] = useState<string | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const dragCounterRef = useRef(0);
  const toInputRef = useRef<HTMLInputElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const lastSavedHashRef = useRef<string>("");

  const hasContent = useCallback(() => {
    return !!(to.trim() || cc.trim() || bcc.trim() || subject.trim() || stripHtml(body).trim() || attachments.length > 0);
  }, [to, cc, bcc, subject, body, attachments]);

  // Auto-focus the To field when dialog opens
  useEffect(() => {
    if (isOpen && toInputRef.current) {
      // Small delay to let the dialog animation complete
      const t = setTimeout(() => toInputRef.current?.focus(), 100);
      return () => clearTimeout(t);
    }
  }, [isOpen]);

  // Compute a simple hash of compose fields for dirty tracking
  const computeHash = useCallback(() => {
    return `${to}|${cc}|${bcc}|${subject}|${body}`;
  }, [to, cc, bcc, subject, body]);

  // Save draft function. When force=true, skip the hash guard (used for explicit save/close).
  const saveDraft = useCallback((force = false) => {
    const hash = computeHash();
    // Don't save if nothing changed (unless forced) or compose is empty
    if (!force && hash === lastSavedHashRef.current) return;
    if (!to.trim() && !cc.trim() && !bcc.trim() && !subject.trim() && !stripHtml(body).trim()) return;

    let currentDraftId = draftId;
    if (!currentDraftId) {
      currentDraftId = generateId();
      setDraftId(currentDraftId);
    }

    saveDraftMutation.mutate(
      {
        id: currentDraftId,
        to,
        cc,
        bcc,
        subject,
        textBody: stripHtml(body),
        htmlBody: body,
        inReplyTo: inReplyTo,
        references: references,
      },
      {
        onSuccess: () => {
          lastSavedHashRef.current = hash;
          setDraftSaved(true);
          setTimeout(() => setDraftSaved(false), 3000);
          toast.success("Draft saved");
        },
      },
    );
  }, [computeHash, to, cc, bcc, subject, body, draftId, setDraftId, inReplyTo, references, saveDraftMutation]);

  // Auto-save every 30s when dialog is open
  useEffect(() => {
    if (!isOpen) return;
    const interval = setInterval(() => {
      saveDraft();
    }, 30000);
    return () => clearInterval(interval);
  }, [isOpen, saveDraft]);

  // Reset saved hash when dialog opens
  useEffect(() => {
    if (isOpen) {
      lastSavedHashRef.current = computeHash();
    }
  }, [isOpen]); // eslint-disable-line react-hooks/exhaustive-deps

  const doSend = useCallback(() => {
    const plainText = stripHtml(body);
    // Convert preview URLs back to cid: references for the email MIME body
    const sendHtml = body.replace(
      /\/api\/drafts\/[^/]+\/attachments\/([^/]+)\/content/g,
      (_match, attId) => `cid:${attId}`,
    );
    sendMutation.mutate(
      { to, cc, bcc, subject, body: plainText, htmlBody: sendHtml, inReplyTo, references, draftId },
      {
        onSuccess: () => {
          toast.success("Message sent");
          reset();
        },
        onError: (error) => {
          toast.error(`Failed to send: ${error.message}`);
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

    // 5-second undo window via sonner
    const timer = setTimeout(() => {
      toast.dismiss("undo-send");
      doSend();
    }, 5000);

    toast("Sending message...", {
      id: "undo-send",
      duration: 5500,
      action: {
        label: "Undo",
        onClick: () => {
          clearTimeout(timer);
          setSending(false);
          useComposeStore.setState({ isOpen: true });
        },
      },
    });
  }, [to, cc, bcc, closeCompose, doSend]);

  const handleDiscard = useCallback(() => {
    if (hasContent()) {
      setShowDiscardAlert(true);
    } else {
      reset();
    }
  }, [hasContent, reset]);

  const confirmDiscard = useCallback(() => {
    setShowDiscardAlert(false);
    if (draftId) {
      deleteDraftMutation.mutate(draftId);
    }
    reset();
  }, [draftId, deleteDraftMutation, reset]);

  // Save draft and close the dialog without discarding.
  const handleSaveAndClose = useCallback(() => {
    if (hasContent()) {
      saveDraft(true);
    }
    closeCompose();
  }, [hasContent, saveDraft, closeCompose]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        handleSend();
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        saveDraft(true);
      }
    },
    [handleSend, saveDraft],
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
            toast.error(`Upload failed: ${error.message}`);
          },
        },
      );

      // Reset the input so the same file can be re-selected
      e.target.value = "";
    },
    [draftId, setDraftId, uploadMutation, addAttachments],
  );

  const handleImageUpload = useCallback(
    async (file: File): Promise<string | null> => {
      // Ensure we have a draft ID for the upload
      let currentDraftId = draftId;
      if (!currentDraftId) {
        currentDraftId = generateId();
        setDraftId(currentDraftId);
      }

      return new Promise((resolve) => {
        uploadMutation.mutate(
          { draftId: currentDraftId!, files: [file] },
          {
            onSuccess: (data) => {
              if (data.attachments.length > 0) {
                const att = data.attachments[0];
                addAttachments([
                  {
                    id: att.id,
                    filename: att.filename,
                    contentType: att.content_type,
                    size: att.size,
                  },
                ]);
                // Return a preview URL that the browser can render.
                // The send flow converts these back to cid: references.
                resolve(`/api/drafts/${currentDraftId}/attachments/${att.id}/content`);
              } else {
                resolve(null);
              }
            },
            onError: (error) => {
              toast.error(`Image upload failed: ${error.message}`);
              resolve(null);
            },
          },
        );
      });
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
            toast.error(`Delete failed: ${error.message}`);
          },
        },
      );
    },
    [draftId, deleteMutation, removeAttachment],
  );

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounterRef.current += 1;
    if (e.dataTransfer.types.includes("Files")) {
      setIsDragging(true);
    }
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounterRef.current -= 1;
    if (dragCounterRef.current === 0) {
      setIsDragging(false);
    }
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);
      dragCounterRef.current = 0;

      const files = Array.from(e.dataTransfer.files);
      if (files.length === 0) return;

      let currentDraftId = draftId;
      if (!currentDraftId) {
        currentDraftId = generateId();
        setDraftId(currentDraftId);
      }

      uploadMutation.mutate(
        { draftId: currentDraftId, files },
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
            toast.success(`${data.attachments.length} file(s) attached`);
          },
          onError: (error) => {
            toast.error(`Upload failed: ${error.message}`);
          },
        },
      );
    },
    [draftId, setDraftId, uploadMutation, addAttachments],
  );

  return (
    <>
      <Dialog.Root
        open={isOpen}
        onOpenChange={(open) => {
          if (!open) {
            handleSaveAndClose();
          }
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-40 bg-black/40" />
          <Dialog.Content
            className={cn(
              "fixed z-50 flex flex-col rounded-xl border border-border bg-background shadow-2xl",
              expanded
                ? "inset-4 sm:left-20"
                : "inset-x-4 bottom-4 top-auto mx-auto max-h-[80vh] w-full max-w-2xl sm:inset-x-auto sm:bottom-8 sm:ml-20",
            )}
            onKeyDown={handleKeyDown}
            onDragEnter={handleDragEnter}
            onDragLeave={handleDragLeave}
            onDragOver={handleDragOver}
            onDrop={handleDrop}
          >
            {/* Drop overlay */}
            {isDragging && (
              <div className="absolute inset-0 z-10 flex flex-col items-center justify-center rounded-xl bg-background/90 backdrop-blur-sm">
                <Upload className="size-10 text-primary" />
                <p className="mt-3 text-sm font-medium text-foreground">
                  Drop files to attach
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  Files will be uploaded as attachments
                </p>
              </div>
            )}

            {/* Header */}
            <div className="flex items-center justify-between border-b border-border px-4 py-3">
              <Dialog.Title className="text-sm font-semibold">
                New Message
              </Dialog.Title>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => setExpanded((e) => !e)}
                  className="rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
                  title={expanded ? "Minimize" : "Maximize"}
                >
                  {expanded ? (
                    <Minimize2 className="size-4" />
                  ) : (
                    <Maximize2 className="size-4" />
                  )}
                </button>
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
                    <button
                      onClick={() => setPreviewAttId(att.id)}
                      className="flex items-center gap-1.5 hover:text-foreground"
                      title="Preview"
                    >
                      <Paperclip className="size-3 shrink-0 text-muted-foreground" />
                      <span className="max-w-[150px] truncate" title={att.filename}>
                        {att.filename}
                      </span>
                      <span className="text-muted-foreground">
                        ({formatFileSize(att.size)})
                      </span>
                    </button>
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
            <RichTextEditor
              content={body}
              onChange={(html) => setField("body", html)}
              onImageUpload={handleImageUpload}
              placeholder="Write your message..."
              className="flex-1 overflow-auto"
            />

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
                <button
                  onClick={() => saveDraft(true)}
                  disabled={saveDraftMutation.isPending}
                  className="rounded-lg p-2 text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-50"
                  title="Save draft (Ctrl+S)"
                >
                  {saveDraftMutation.isPending ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <Save className="size-4" />
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
              <div className="flex items-center gap-2">
                {draftSaved && (
                  <span className="text-xs text-muted-foreground">
                    Draft saved
                  </span>
                )}
                <button
                  onClick={handleDiscard}
                  className="rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
                >
                  Discard
                </button>
              </div>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      <AlertDialog.Root open={showDiscardAlert} onOpenChange={setShowDiscardAlert}>
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="fixed inset-0 z-50 bg-black/40" />
          <AlertDialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl border border-border bg-background p-6 shadow-2xl">
            <AlertDialog.Title className="text-base font-semibold">
              Are you sure?
            </AlertDialog.Title>
            <AlertDialog.Description className="mt-2 text-sm text-muted-foreground">
              The message has not been sent and has unsaved changes. Do you want to discard your changes?
            </AlertDialog.Description>
            <div className="mt-6 flex justify-end gap-3">
              <AlertDialog.Cancel asChild>
                <button className="rounded-lg px-4 py-2 text-sm font-medium text-muted-foreground hover:bg-accent hover:text-foreground">
                  Cancel
                </button>
              </AlertDialog.Cancel>
              <AlertDialog.Action asChild>
                <button
                  onClick={confirmDiscard}
                  className="rounded-lg bg-destructive px-4 py-2 text-sm font-medium text-destructive-foreground hover:bg-destructive/90"
                >
                  Discard
                </button>
              </AlertDialog.Action>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>

      {/* Attachment preview dialog */}
      {previewAttId && draftId && (() => {
        const att = attachments.find((a) => a.id === previewAttId);
        if (!att) return null;
        const previewUrl = `/api/drafts/${draftId}/attachments/${att.id}/content`;
        return (
          <Dialog.Root open onOpenChange={(open) => !open && setPreviewAttId(null)}>
            <Dialog.Portal>
              <Dialog.Overlay className="fixed inset-0 z-[60] bg-black/70" />
              <Dialog.Content className="fixed inset-4 z-[60] flex flex-col rounded-xl border border-border bg-background shadow-2xl">
                <div className="flex items-center justify-between border-b border-border px-4 py-3">
                  <Dialog.Title className="flex items-center gap-2 text-sm font-semibold">
                    <Paperclip className="size-4 text-muted-foreground" />
                    <span className="max-w-[400px] truncate">{att.filename}</span>
                    <span className="text-xs font-normal text-muted-foreground">
                      ({formatFileSize(att.size)})
                    </span>
                  </Dialog.Title>
                  <div className="flex items-center gap-1">
                    <a
                      href={previewUrl}
                      download={att.filename}
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
                <div className="flex flex-1 items-center justify-center overflow-auto p-4">
                  {att.contentType.startsWith("image/") ? (
                    <img
                      src={previewUrl}
                      alt={att.filename}
                      className="max-h-full max-w-full object-contain"
                    />
                  ) : att.contentType === "application/pdf" ? (
                    <iframe
                      src={previewUrl}
                      className="h-full w-full border-none"
                      title={att.filename}
                    />
                  ) : (
                    <div className="flex flex-col items-center gap-4 text-center">
                      <Paperclip className="size-12 text-muted-foreground" />
                      <p className="text-sm text-muted-foreground">
                        Preview not available for this file type
                      </p>
                      <a
                        href={previewUrl}
                        download={att.filename}
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
      })()}
    </>
  );
}
