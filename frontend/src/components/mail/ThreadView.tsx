"use client";

import { useState, useRef, useEffect } from "react";
import { ChevronDown, ChevronUp, Paperclip } from "lucide-react";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/useUiStore";
import type { MessageHeader } from "@/types/message";

interface ThreadViewProps {
  thread: MessageHeader[];
  currentUid: number;
}

/** Deterministic color palette for initials avatars. */
const AVATAR_COLORS = [
  "bg-blue-600",
  "bg-emerald-600",
  "bg-violet-600",
  "bg-amber-600",
  "bg-rose-600",
  "bg-cyan-600",
  "bg-fuchsia-600",
  "bg-teal-600",
  "bg-orange-600",
  "bg-indigo-600",
] as const;

/** Derive up to two initials from a display name or email address. */
function getInitials(name: string, email: string): string {
  const source = name || email;
  if (!source) return "?";

  // If it looks like an email, use the first letter of the local part.
  if (!name && source.includes("@")) {
    return source.charAt(0).toUpperCase();
  }

  const parts = source.trim().split(/\s+/);
  if (parts.length === 1) return parts[0].charAt(0).toUpperCase();
  return (parts[0].charAt(0) + parts[parts.length - 1].charAt(0)).toUpperCase();
}

/** Pick a consistent color based on the sender identifier. */
function avatarColor(identifier: string): string {
  let hash = 0;
  for (let i = 0; i < identifier.length; i++) {
    hash = (hash * 31 + identifier.charCodeAt(i)) | 0;
  }
  return AVATAR_COLORS[Math.abs(hash) % AVATAR_COLORS.length];
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
  const scrollRef = useRef<HTMLDivElement>(null);
  const currentRef = useRef<HTMLButtonElement>(null);

  // Track which UIDs are expanded — current message is expanded by default.
  const [expandedUids, setExpandedUids] = useState<Set<number>>(
    () => new Set([currentUid]),
  );

  // Keep expanded set in sync when currentUid changes from outside.
  // We do this lazily inside render — safe because we only call setState
  // when the value actually diverges.
  if (!expandedUids.has(currentUid)) {
    setExpandedUids(new Set([currentUid]));
  }

  // Auto-scroll to the current message when the thread loads.
  useEffect(() => {
    requestAnimationFrame(() => {
      currentRef.current?.scrollIntoView({ block: "nearest" });
    });
  }, [currentUid]);

  function toggleExpand(uid: number) {
    setExpandedUids((prev) => {
      const next = new Set(prev);
      if (next.has(uid)) {
        next.delete(uid);
      } else {
        next.add(uid);
      }
      return next;
    });
  }

  // Determine the "main" subject so we can hide it on cards that share it.
  const mainSubject = thread[0]?.subject ?? "";

  return (
    <div className="shrink-0 border-b border-border">
      {/* Conversation header */}
      <div className="flex items-center gap-2 px-4 py-2">
        <span className="text-xs font-medium text-muted-foreground">
          Conversation
        </span>
        <span className="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-muted px-1.5 text-[11px] font-semibold text-muted-foreground">
          {thread.length}
        </span>
      </div>

      {/* Message cards — scrollable with a max height */}
      <div ref={scrollRef} className="flex max-h-60 flex-col gap-px overflow-y-auto bg-border">
        {thread.map((msg) => {
          const isExpanded = expandedUids.has(msg.uid);
          const isCurrent = msg.uid === currentUid;
          const sender = msg.from_name || msg.from_address;
          const initials = getInitials(msg.from_name, msg.from_address);
          const color = avatarColor(msg.from_address);

          // Show subject line only when it differs from the thread subject.
          const normalise = (s: string) =>
            s.replace(/^(?:re|fwd?):\s*/i, "").trim().toLowerCase();
          const showSubject = normalise(msg.subject) !== normalise(mainSubject);

          return (
            <button
              key={`${msg.folder}-${msg.uid}`}
              ref={isCurrent ? currentRef : undefined}
              type="button"
              onClick={() => {
                if (!isExpanded) {
                  // Expand and navigate to this message.
                  toggleExpand(msg.uid);
                  if (!isCurrent) selectMessage(msg.uid);
                } else {
                  // Already expanded — toggle collapse (but don't collapse current).
                  toggleExpand(msg.uid);
                }
              }}
              className={cn(
                "flex w-full gap-3 bg-background px-4 text-left transition-colors",
                isExpanded ? "py-3" : "py-2",
                isCurrent && "bg-accent/50",
                !isExpanded && "cursor-pointer hover:bg-muted/50",
              )}
            >
              {/* Avatar */}
              <div
                className={cn(
                  "flex shrink-0 items-center justify-center rounded-full text-[11px] font-semibold text-white",
                  isExpanded ? "size-8" : "size-6",
                  color,
                )}
              >
                {initials}
              </div>

              {/* Content */}
              <div className="min-w-0 flex-1">
                {isExpanded ? (
                  /* ── Expanded card ── */
                  <div className="space-y-0.5">
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <span className="text-sm font-semibold text-foreground">
                          {sender}
                        </span>
                        {msg.from_name && (
                          <span className="ml-1.5 text-xs text-muted-foreground">
                            &lt;{msg.from_address}&gt;
                          </span>
                        )}
                      </div>
                      <div className="flex shrink-0 items-center gap-1.5">
                        <span className="text-xs text-muted-foreground">
                          {formatThreadDate(msg.date)}
                        </span>
                        <ChevronUp className="size-3.5 text-muted-foreground" />
                      </div>
                    </div>

                    {showSubject && (
                      <div className="text-xs text-muted-foreground">
                        {msg.subject}
                      </div>
                    )}

                    {/* Snippet preview */}
                    {msg.snippet && (
                      <p className="line-clamp-2 text-sm leading-relaxed text-muted-foreground">
                        {msg.snippet}
                      </p>
                    )}

                    {msg.has_attachments && (
                      <div className="flex items-center gap-1 pt-0.5 text-xs text-muted-foreground">
                        <Paperclip className="size-3" />
                        <span>Attachment</span>
                      </div>
                    )}

                    {isCurrent && (
                      <div className="pt-0.5 text-[11px] font-medium text-primary">
                        Currently viewing
                      </div>
                    )}
                  </div>
                ) : (
                  /* ── Collapsed card ── */
                  <div className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate text-sm">
                      <span className="font-medium text-foreground">
                        {sender}
                      </span>
                      {msg.snippet && (
                        <span className="text-muted-foreground">
                          {" "}&mdash; {msg.snippet}
                        </span>
                      )}
                    </span>
                    <div className="flex shrink-0 items-center gap-1.5">
                      {msg.has_attachments && (
                        <Paperclip className="size-3 text-muted-foreground" />
                      )}
                      <span className="text-xs text-muted-foreground">
                        {formatThreadDate(msg.date)}
                      </span>
                      <ChevronDown className="size-3.5 text-muted-foreground" />
                    </div>
                  </div>
                )}
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
