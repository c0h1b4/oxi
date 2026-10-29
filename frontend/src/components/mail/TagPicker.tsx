"use client";

import { Popover } from "radix-ui";

import { useState } from "react";
import { Tag, Check } from "lucide-react";
import { cn } from "@/lib/utils";
import {
  useTags,
  useMessageTags,
  useAddTagToMessage,
  useRemoveTagFromMessage,
} from "@/hooks/useTags";

interface TagPickerProps {
  folder: string;
  uid: number;
}

export function TagPicker({ folder, uid }: TagPickerProps) {
  const [isOpen, setIsOpen] = useState(false);

  const { data: tagsData } = useTags();
  const { data: messageTagsData } = useMessageTags(folder, uid);
  const addTag = useAddTagToMessage();
  const removeTag = useRemoveTagFromMessage();

  const allTags = tagsData?.tags ?? [];
  const appliedTagIds = new Set(
    (messageTagsData?.tags ?? []).map((t) => t.id),
  );


  const handleToggle = (tagId: string) => {
    if (appliedTagIds.has(tagId)) {
      removeTag.mutate({ tagId, messageUid: uid, messageFolder: folder });
    } else {
      addTag.mutate({ tagId, messageUid: uid, messageFolder: folder });
    }
  };

  return (
    <Popover.Root open={isOpen} onOpenChange={setIsOpen}>
      <div className="relative">
      <Popover.Trigger asChild>
      <button
        type="button"
        aria-label="Tags"
        className={cn(
          "inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
          "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
          isOpen && "bg-accent text-accent-foreground",
        )}
      >
        <Tag className="size-4" />
        <span className="hidden xl:inline">Tags</span>
      </button>
      </Popover.Trigger>

      {isOpen && (
        <Popover.Portal><Popover.Content sideOffset={4} collisionPadding={8} className="z-50 max-h-[var(--radix-popover-content-available-height)] w-48 max-w-[calc(100vw-1rem)] overflow-y-auto rounded-md border border-border bg-popover py-1 shadow-md">
          {allTags.length === 0 && (
            <span className="block px-3 py-1.5 text-sm text-muted-foreground">
              No tags yet
            </span>
          )}
          {allTags.map((tag) => {
            const isApplied = appliedTagIds.has(tag.id);
            return (
              <button
                key={tag.id}
                type="button"
                onClick={() => handleToggle(tag.id)}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-accent"
              >
                <span
                  className="size-2.5 shrink-0 rounded-full"
                  style={{ backgroundColor: tag.color }}
                />
                <span className="flex-1 truncate">{tag.name}</span>
                {isApplied && (
                  <Check className="size-3.5 shrink-0 text-primary" />
                )}
              </button>
            );
          })}
        </Popover.Content></Popover.Portal>
      )}
      </div>
    </Popover.Root>
  );
}
