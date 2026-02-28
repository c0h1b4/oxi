"use client";

import { Dialog } from "radix-ui";
import { Paperclip, X, Download } from "lucide-react";
import { formatFileSize } from "./utils";

interface Attachment {
  id: string;
  filename: string;
  contentType: string;
  size: number;
}

interface AttachmentPreviewDialogProps {
  attachment: Attachment;
  previewUrl: string;
  onClose: () => void;
}

export function AttachmentPreviewDialog({
  attachment,
  previewUrl,
  onClose,
}: AttachmentPreviewDialogProps) {
  return (
    <Dialog.Root open onOpenChange={(open) => !open && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-[60] bg-black/70" />
        <Dialog.Content className="fixed inset-4 z-[60] flex flex-col rounded-xl border border-border bg-background shadow-2xl">
          <div className="flex items-center justify-between border-b border-border px-4 py-3">
            <Dialog.Title className="flex items-center gap-2 text-sm font-semibold">
              <Paperclip className="size-4 text-muted-foreground" />
              <span className="max-w-[400px] truncate">{attachment.filename}</span>
              <span className="text-xs font-normal text-muted-foreground">
                ({formatFileSize(attachment.size)})
              </span>
            </Dialog.Title>
            <div className="flex items-center gap-1">
              <a
                href={previewUrl}
                download={attachment.filename}
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
            {attachment.contentType.startsWith("image/") ? (
              <img
                src={previewUrl}
                alt={attachment.filename}
                className="max-h-full max-w-full object-contain"
              />
            ) : attachment.contentType === "application/pdf" ? (
              <iframe
                src={previewUrl}
                className="h-full w-full border-none"
                title={attachment.filename}
              />
            ) : (
              <div className="flex flex-col items-center gap-4 text-center">
                <Paperclip className="size-12 text-muted-foreground" />
                <p className="text-sm text-muted-foreground">
                  Preview not available for this file type
                </p>
                <a
                  href={previewUrl}
                  download={attachment.filename}
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
