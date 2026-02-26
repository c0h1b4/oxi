"use client";

import { memo, useState, useCallback } from "react";
import { Star, Paperclip } from "lucide-react";
import { cn } from "@/lib/utils";
import { useUpdateFlags } from "@/hooks/useMessages";
import type { MessageHeader } from "@/types/message";

interface MessageListItemProps {
  message: MessageHeader;
  isSelected: boolean;
  density: "compact" | "comfortable";
  onClick: () => void;
  bulkSelectMode: boolean;
  isBulkSelected: boolean;
  onBulkToggle: (uid: number) => void;
}

function formatDate(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();

  // Guard against invalid dates
  if (isNaN(date.getTime())) return dateStr;

  const isToday =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();

  if (isToday) {
    return date.toLocaleTimeString(undefined, {
      hour: "numeric",
      minute: "2-digit",
    });
  }

  // Check if same week (within last 7 days)
  const msPerDay = 86_400_000;
  const daysDiff = Math.floor(
    (now.getTime() - date.getTime()) / msPerDay,
  );

  const time = date.toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });

  if (daysDiff < 7 && daysDiff >= 0) {
    const day = date.toLocaleDateString(undefined, { weekday: "short" });
    return `${day} ${time}`;
  }

  // Same year
  if (date.getFullYear() === now.getFullYear()) {
    const day = date.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
    return `${day} ${time}`;
  }

  // Older
  const day = date.toLocaleDateString(undefined, {
    month: "2-digit",
    day: "2-digit",
    year: "2-digit",
  });
  return `${day} ${time}`;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${Math.round(kb)} KB`;
  const mb = kb / 1024;
  return `${mb.toFixed(1)} MB`;
}

export const MessageListItem = memo(function MessageListItem({
  message,
  isSelected,
  density,
  onClick,
  bulkSelectMode,
  isBulkSelected,
  onBulkToggle,
}: MessageListItemProps) {
  const isUnread = !message.flags.includes("\\Seen");
  const isFlagged = message.flags.includes("\\Flagged");
  const sender = message.from_name || message.from_address;
  const formattedDate = formatDate(message.date);
  const formattedSize = formatSize(message.size);
  const updateFlags = useUpdateFlags();
  const [isHovered, setIsHovered] = useState(false);

  const toggleStar = (e: React.MouseEvent) => {
    e.stopPropagation();
    updateFlags.mutate({
      folder: message.folder,
      uid: message.uid,
      flags: ["\\Flagged"],
      add: !isFlagged,
    });
  };

  const toggleRead = (e: React.MouseEvent) => {
    e.stopPropagation();
    updateFlags.mutate({
      folder: message.folder,
      uid: message.uid,
      flags: ["\\Seen"],
      add: isUnread, // if unread, add \Seen; if read, remove \Seen
    });
  };

  const [isDragging, setIsDragging] = useState(false);

  const handleDragStart = useCallback(
    (e: React.DragEvent) => {
      e.dataTransfer.setData(
        "application/json",
        JSON.stringify({
          uid: message.uid,
          folder: message.folder,
          subject: message.subject,
        }),
      );
      e.dataTransfer.effectAllowed = "move";
      setIsDragging(true);
    },
    [message.uid, message.folder, message.subject],
  );

  const handleDragEnd = useCallback(() => {
    setIsDragging(false);
  }, []);

  if (density === "compact") {
    return (
      <div
        role="row"
        aria-selected={isSelected}
        tabIndex={0}
        onClick={onClick}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onClick();
          }
        }}
        draggable
        onDragStart={handleDragStart}
        onDragEnd={handleDragEnd}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
        className={cn(
          "flex h-9 cursor-pointer items-center gap-2 border-b border-border px-3 text-sm transition-colors",
          "hover:bg-muted",
          isUnread ? "bg-background font-semibold" : "bg-transparent font-normal",
          isSelected && "bg-accent hover:bg-accent",
          isDragging && "opacity-50",
        )}
      >
        {/* Bulk checkbox or unread indicator dot */}
        {bulkSelectMode ? (
          <button
            type="button"
            aria-label={isBulkSelected ? "Deselect" : "Select"}
            onClick={(e) => { e.stopPropagation(); onBulkToggle(message.uid); }}
            className={cn(
              "flex size-4 shrink-0 items-center justify-center rounded border transition-colors",
              isBulkSelected
                ? "border-primary bg-primary text-primary-foreground"
                : "border-muted-foreground/40 bg-transparent hover:border-primary",
            )}
          >
            {isBulkSelected && (
              <svg className="size-3" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M2.5 6l2.5 2.5 4.5-5" />
              </svg>
            )}
          </button>
        ) : (
          <button
            type="button"
            aria-label={isUnread ? "Mark as read" : "Mark as unread"}
            onClick={toggleRead}
            className="flex size-4 shrink-0 items-center justify-center rounded-full hover:bg-muted-foreground/20"
          >
            <span
              className={cn(
                "size-1.5 rounded-full",
                isUnread ? "bg-primary" : "bg-border",
              )}
            />
          </button>
        )}

        {/* Star */}
        <button
          type="button"
          aria-label={isFlagged ? "Unstar" : "Star"}
          onClick={toggleStar}
          className="flex size-4 shrink-0 items-center justify-center rounded-sm hover:bg-muted-foreground/20"
        >
          {isFlagged ? (
            <Star className="size-3.5 fill-primary text-primary" />
          ) : (
            <Star className="size-3.5 text-muted-foreground/40" />
          )}
        </button>

        {/* Sender */}
        <span className={cn("w-32 shrink-0 truncate font-normal", isFlagged ? "text-primary" : "text-muted-foreground")}>{sender}</span>

        {/* Dash separator */}
        <span className="shrink-0 text-muted-foreground">&mdash;</span>

        {/* Subject */}
        <span className={cn("min-w-0 flex-1 truncate", isFlagged && "text-primary")}>{message.subject || "(no subject)"}</span>

        {/* Attachment icon */}
        {message.has_attachments && (
          <Paperclip className="size-3.5 shrink-0 text-muted-foreground" />
        )}

        {/* Date / Size on hover */}
        <span className={cn("shrink-0 text-xs", isFlagged ? "text-primary" : "text-muted-foreground")}>
          {isHovered ? formattedSize : formattedDate}
        </span>
      </div>
    );
  }

  // Comfortable layout
  return (
    <div
      role="row"
      aria-selected={isSelected}
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      draggable
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      className={cn(
        "flex h-16 cursor-pointer flex-col justify-center border-b border-border px-3 py-1.5 transition-colors",
        "hover:bg-muted",
        isUnread ? "bg-background" : "bg-transparent",
        isSelected && "bg-accent hover:bg-accent",
        isDragging && "opacity-50",
      )}
    >
      {/* Top row: sender + date + star */}
      <div className="flex items-center gap-2 pl-6">
        {/* Sender name */}
        <span className={cn("min-w-0 flex-1 truncate text-xs font-normal", isFlagged ? "text-primary" : "text-muted-foreground")}>{sender}</span>

        {/* Date / Size on hover */}
        <span className={cn("shrink-0 text-xs font-normal", isFlagged ? "text-primary" : "text-muted-foreground")}>
          {isHovered ? formattedSize : formattedDate}
        </span>

        {/* Star */}
        <button
          type="button"
          aria-label={isFlagged ? "Unstar" : "Star"}
          onClick={toggleStar}
          className="flex size-4 shrink-0 items-center justify-center rounded-sm hover:bg-muted-foreground/20"
        >
          {isFlagged ? (
            <Star className="size-3.5 fill-primary text-primary" />
          ) : (
            <Star className="size-3.5 text-muted-foreground/40" />
          )}
        </button>
      </div>

      {/* Bottom row: checkbox/dot + subject + snippet */}
      <div className="flex items-center gap-2">
        {/* Bulk checkbox or unread indicator dot */}
        {bulkSelectMode ? (
          <button
            type="button"
            aria-label={isBulkSelected ? "Deselect" : "Select"}
            onClick={(e) => { e.stopPropagation(); onBulkToggle(message.uid); }}
            className={cn(
              "flex size-4 shrink-0 items-center justify-center rounded border transition-colors",
              isBulkSelected
                ? "border-primary bg-primary text-primary-foreground"
                : "border-muted-foreground/40 bg-transparent hover:border-primary",
            )}
          >
            {isBulkSelected && (
              <svg className="size-3" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M2.5 6l2.5 2.5 4.5-5" />
              </svg>
            )}
          </button>
        ) : (
          <button
            type="button"
            aria-label={isUnread ? "Mark as read" : "Mark as unread"}
            onClick={toggleRead}
            className="flex size-4 shrink-0 items-center justify-center rounded-full hover:bg-muted-foreground/20"
          >
            <span
              className={cn(
                "size-1.5 rounded-full",
                isUnread ? "bg-primary" : "bg-border",
              )}
            />
          </button>
        )}

        <span className="min-w-0 flex-1 truncate text-sm">
          <span className={cn(isUnread ? "font-medium" : "font-normal", isFlagged ? "text-primary" : "text-foreground")}>
            {message.subject || "(no subject)"}
          </span>
          {message.snippet && (
            <span className="font-normal text-muted-foreground">
              {" "}
              &mdash; {message.snippet}
            </span>
          )}
        </span>

        {/* Attachment icon */}
        {message.has_attachments && (
          <Paperclip className="size-3.5 shrink-0 text-muted-foreground" />
        )}
      </div>
    </div>
  );
});
