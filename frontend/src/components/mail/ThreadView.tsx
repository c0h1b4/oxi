"use client";

import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/useUiStore";
import type { MessageHeader } from "@/types/message";

interface ThreadViewProps {
  thread: MessageHeader[];
  currentUid: number;
}

function formatThreadDate(dateStr: string): string {
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return dateStr;

  return date.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });
}

export function ThreadView({ thread, currentUid }: ThreadViewProps) {
  const selectMessage = useUiStore((s) => s.selectMessage);

  return (
    <div className="shrink-0 border-b border-border">
      <div className="px-4 py-2 text-xs font-medium text-muted-foreground">
        {thread.length} messages in thread
      </div>
      <div className="flex flex-col">
        {thread.map((msg) => {
          const isCurrent = msg.uid === currentUid;
          const sender = msg.from_name || msg.from_address;

          return (
            <button
              key={`${msg.folder}-${msg.uid}`}
              type="button"
              onClick={() => {
                if (!isCurrent) {
                  selectMessage(msg.uid);
                }
              }}
              className={cn(
                "flex items-center gap-3 border-t border-border px-4 py-2 text-left text-sm transition-colors",
                isCurrent
                  ? "bg-accent font-medium"
                  : "cursor-pointer hover:bg-muted",
              )}
            >
              <span className="min-w-0 flex-1 truncate">
                <span className={cn(isCurrent && "font-semibold")}>
                  {sender}
                </span>
                <span className="text-muted-foreground">
                  {" "}&mdash; {msg.subject}
                </span>
              </span>
              <span className="shrink-0 text-xs text-muted-foreground">
                {formatThreadDate(msg.date)}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
