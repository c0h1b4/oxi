"use client";

import { useState, useEffect, useCallback } from "react";
import { Dialog } from "radix-ui";
import {
  Paperclip,
  X,
  Download,
  ChevronLeft,
  ChevronRight,
  FileText,
  File,
  CalendarDays,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { formatFileSize } from "./utils";
import { IcsPreview } from "./IcsPreview";
import type { Attachment } from "@/types/message";

function isCalendarType(ct: string): boolean {
  return ct === "text/calendar" || ct === "application/ics";
}

function isPdfType(ct: string): boolean {
  return ct.toLowerCase() === "application/pdf";
}

interface AttachmentPreviewerProps {
  attachments: Attachment[];
  baseUrl: string;
  initialIndex: number;
  onClose: () => void;
}

export function AttachmentPreviewer({
  attachments,
  baseUrl,
  initialIndex,
  onClose,
}: AttachmentPreviewerProps) {
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
                    ) : isPdfType(thumb.content_type) ? (
                      <FileText className="size-6 text-muted-foreground" />
                    ) : isCalendarType(thumb.content_type) ? (
                      <CalendarDays className="size-6 text-muted-foreground" />
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
            ) : isPdfType(att.content_type) ? (
              <iframe
                src={url}
                className="h-full w-full border-none"
                title={att.filename ?? "PDF"}
              />
            ) : isCalendarType(att.content_type) ? (
              <IcsPreview url={url} filename={att.filename} />
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
