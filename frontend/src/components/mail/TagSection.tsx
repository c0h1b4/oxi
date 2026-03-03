"use client";

import { useState, useRef, useEffect } from "react";
import { ChevronRight, Plus, X, Loader2 } from "lucide-react";
import { useTags, useCreateTag, useDeleteTag } from "@/hooks/useTags";
import { useUiStore } from "@/stores/useUiStore";
import { cn } from "@/lib/utils";

const PRESET_COLORS = [
  "#ef4444", // red
  "#f97316", // orange
  "#eab308", // yellow
  "#22c55e", // green
  "#06b6d4", // cyan
  "#3b82f6", // blue
  "#8b5cf6", // violet
  "#ec4899", // pink
  "#6b7280", // gray
];

export function TagSection() {
  const { data, isLoading } = useTags();
  const createTag = useCreateTag();
  const deleteTag = useDeleteTag();
  const activeTagId = useUiStore((s) => s.activeTagId);
  const setActiveTag = useUiStore((s) => s.setActiveTag);

  const [isCollapsed, setIsCollapsed] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [newColor, setNewColor] = useState(PRESET_COLORS[5]);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isCreating) {
      inputRef.current?.focus();
    }
  }, [isCreating]);

  const handleCreate = () => {
    const trimmed = newName.trim();
    if (!trimmed) return;
    createTag.mutate(
      { name: trimmed, color: newColor },
      {
        onSuccess: () => {
          setNewName("");
          setNewColor(PRESET_COLORS[5]);
          setIsCreating(false);
        },
      },
    );
  };

  const tags = data?.tags ?? [];

  return (
    <div className="mt-2 border-t border-sidebar-border pt-2">
      {/* Section header */}
      <button
        onClick={() => setIsCollapsed((prev) => !prev)}
        className="flex w-full items-center gap-1 px-3 py-1 text-xs font-semibold uppercase tracking-wider text-muted-foreground hover:text-foreground"
      >
        <ChevronRight
          className={cn(
            "size-3 transition-transform",
            !isCollapsed && "rotate-90",
          )}
        />
        <span className="flex-1 text-left">Tags</span>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            setIsCreating(true);
            setIsCollapsed(false);
          }}
          className="rounded p-0.5 hover:bg-sidebar-accent"
          title="Create tag"
        >
          <Plus className="size-3" />
        </button>
      </button>

      {!isCollapsed && (
        <div className="flex flex-col gap-0.5 pt-0.5">
          {isLoading && (
            <div className="flex items-center justify-center py-2">
              <Loader2 className="size-3.5 animate-spin text-muted-foreground" />
            </div>
          )}

          {/* Tag items */}
          {tags.map((tag) => (
            <button
              key={tag.id}
              onClick={() =>
                setActiveTag(activeTagId === tag.id ? null : tag.id)
              }
              className={cn(
                "group flex w-full items-center gap-2.5 rounded-md px-3 py-1.5 text-sm transition-colors",
                activeTagId === tag.id
                  ? "bg-primary/10 font-semibold text-primary"
                  : "font-medium text-sidebar-foreground hover:bg-sidebar-accent",
              )}
            >
              <span
                className="size-2.5 shrink-0 rounded-full"
                style={{ backgroundColor: tag.color }}
              />
              <span className="flex-1 truncate text-left">{tag.name}</span>
              {tag.message_count > 0 && (
                <span className="text-xs text-muted-foreground">
                  {tag.message_count}
                </span>
              )}
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  deleteTag.mutate(tag.id);
                }}
                className="hidden rounded p-0.5 text-muted-foreground hover:text-destructive group-hover:block"
                title="Delete tag"
              >
                <X className="size-3" />
              </button>
            </button>
          ))}

          {/* Inline create form */}
          {isCreating && (
            <div className="flex flex-col gap-1.5 px-3 py-1.5">
              <div className="flex items-center gap-1.5">
                <input
                  ref={inputRef}
                  type="text"
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleCreate();
                    if (e.key === "Escape") {
                      setIsCreating(false);
                      setNewName("");
                    }
                  }}
                  placeholder="Tag name"
                  className="flex-1 rounded border border-input bg-background px-2 py-1 text-sm outline-none focus:border-ring focus:ring-1 focus:ring-ring/50"
                  autoComplete="off"
                  spellCheck={false}
                />
                <button
                  type="button"
                  onClick={() => {
                    setIsCreating(false);
                    setNewName("");
                  }}
                  className="rounded p-0.5 text-muted-foreground hover:text-foreground"
                >
                  <X className="size-3.5" />
                </button>
              </div>

              {/* Color palette */}
              <div className="flex gap-1">
                {PRESET_COLORS.map((color) => (
                  <button
                    key={color}
                    type="button"
                    onClick={() => setNewColor(color)}
                    className={cn(
                      "size-4 rounded-full border-2 transition-transform",
                      newColor === color
                        ? "scale-125 border-foreground"
                        : "border-transparent hover:scale-110",
                    )}
                    style={{ backgroundColor: color }}
                    title={color}
                  />
                ))}
              </div>

              <button
                type="button"
                onClick={handleCreate}
                disabled={!newName.trim() || createTag.isPending}
                className={cn(
                  "rounded-md bg-primary px-2 py-1 text-xs font-medium text-primary-foreground transition-colors",
                  "disabled:opacity-50",
                  "hover:bg-primary/90",
                )}
              >
                {createTag.isPending ? "Creating..." : "Create"}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
