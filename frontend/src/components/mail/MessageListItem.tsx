"use client";

import { memo } from "react";
import { Star, Paperclip } from "lucide-react";
import { cn } from "@/lib/utils";
import type { MessageHeader } from "@/types/message";

interface MessageListItemProps {
  message: MessageHeader;
  isSelected: boolean;
  density: "compact" | "comfortable";
  onClick: () => void;
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

  if (daysDiff < 7 && daysDiff >= 0) {
    return date.toLocaleDateString(undefined, { weekday: "short" });
  }

  // Same year
  if (date.getFullYear() === now.getFullYear()) {
    return date.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
  }

  // Older
  return date.toLocaleDateString(undefined, {
    month: "2-digit",
    day: "2-digit",
    year: "2-digit",
  });
}

export const MessageListItem = memo(function MessageListItem({
  message,
  isSelected,
  density,
  onClick,
}: MessageListItemProps) {
  const isUnread = !message.flags.includes("\\Seen");
  const isFlagged = message.flags.includes("\\Flagged");
  const sender = message.from_name || message.from_address;
  const formattedDate = formatDate(message.date);

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
        className={cn(
          "flex h-9 cursor-pointer items-center gap-2 border-b border-border px-3 text-sm transition-colors",
          "hover:bg-muted",
          isUnread ? "bg-background font-semibold" : "bg-transparent font-normal",
          isSelected && "bg-accent hover:bg-accent",
        )}
      >
        {/* Unread indicator dot */}
        <span
          className={cn(
            "size-1.5 shrink-0 rounded-full",
            isUnread ? "bg-primary" : "bg-transparent",
          )}
        />

        {/* Star */}
        {isFlagged ? (
          <Star className="size-3.5 shrink-0 fill-primary text-primary" />
        ) : (
          <span className="size-3.5 shrink-0" />
        )}

        {/* Sender */}
        <span className="w-32 shrink-0 truncate">{sender}</span>

        {/* Dash separator */}
        <span className="shrink-0 text-muted-foreground">&mdash;</span>

        {/* Subject */}
        <span className="min-w-0 flex-1 truncate">{message.subject}</span>

        {/* Attachment icon */}
        {message.has_attachments && (
          <Paperclip className="size-3.5 shrink-0 text-muted-foreground" />
        )}

        {/* Date */}
        <span className="shrink-0 text-xs text-muted-foreground">
          {formattedDate}
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
      className={cn(
        "flex h-16 cursor-pointer flex-col justify-center border-b border-border px-3 py-1.5 transition-colors",
        "hover:bg-muted",
        isUnread ? "bg-background font-semibold" : "bg-transparent font-normal",
        isSelected && "bg-accent hover:bg-accent",
      )}
    >
      {/* Top row: sender + date */}
      <div className="flex items-center gap-2">
        {/* Unread indicator dot */}
        <span
          className={cn(
            "size-1.5 shrink-0 rounded-full",
            isUnread ? "bg-primary" : "bg-transparent",
          )}
        />

        {/* Star */}
        {isFlagged && (
          <Star className="size-3.5 shrink-0 fill-primary text-primary" />
        )}

        {/* Sender name */}
        <span className="min-w-0 flex-1 truncate text-sm">{sender}</span>

        {/* Date */}
        <span className="shrink-0 text-xs font-normal text-muted-foreground">
          {formattedDate}
        </span>
      </div>

      {/* Bottom row: subject + snippet + attachments */}
      <div className="flex items-center gap-2 pl-[calc(0.375rem+0.375rem)]">
        <span className="min-w-0 flex-1 truncate text-sm font-normal text-muted-foreground">
          <span className={cn(isUnread && "font-medium text-foreground")}>
            {message.subject}
          </span>
          {message.snippet && (
            <span className="text-muted-foreground">
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
