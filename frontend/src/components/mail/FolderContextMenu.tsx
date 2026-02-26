"use client";

import { useState, useRef, useEffect, useCallback, type ReactNode } from "react";
import { Pencil, Trash2 } from "lucide-react";
import { useDeleteFolder } from "@/hooks/useFolders";
import { cn } from "@/lib/utils";

/** System folders that cannot be renamed or deleted. */
const SYSTEM_FOLDERS = new Set([
  "INBOX",
  "Sent",
  "Drafts",
  "Trash",
  "Junk",
  "Spam",
]);

export function isSystemFolder(name: string): boolean {
  return SYSTEM_FOLDERS.has(name);
}

interface FolderContextMenuProps {
  folderName: string;
  onRename: () => void;
  children: ReactNode;
  onDragOver?: (e: React.DragEvent) => void;
  onDragEnter?: (e: React.DragEvent) => void;
  onDragLeave?: (e: React.DragEvent) => void;
  onDrop?: (e: React.DragEvent) => void;
}

export function FolderContextMenu({
  folderName,
  onRename,
  children,
  onDragOver,
  onDragEnter,
  onDragLeave,
  onDrop,
}: FolderContextMenuProps) {
  const [menuPos, setMenuPos] = useState<{ x: number; y: number } | null>(
    null,
  );
  const [confirmDelete, setConfirmDelete] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const deleteFolder = useDeleteFolder();

  const isSystem = isSystemFolder(folderName);

  const closeMenu = useCallback(() => {
    setMenuPos(null);
    setConfirmDelete(false);
  }, []);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      // Don't show context menu for system folders
      if (isSystem) return;

      e.preventDefault();
      e.stopPropagation();
      setMenuPos({ x: e.clientX, y: e.clientY });
      setConfirmDelete(false);
    },
    [isSystem],
  );

  // Close on click outside
  useEffect(() => {
    if (!menuPos) return;
    function handleClick(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        closeMenu();
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [menuPos, closeMenu]);

  // Close on Escape
  useEffect(() => {
    if (!menuPos) return;
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        closeMenu();
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [menuPos, closeMenu]);

  const handleRename = useCallback(() => {
    closeMenu();
    onRename();
  }, [closeMenu, onRename]);

  const handleDelete = useCallback(() => {
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    deleteFolder.mutate(
      { name: folderName },
      { onSuccess: () => closeMenu() },
    );
  }, [confirmDelete, deleteFolder, folderName, closeMenu]);

  return (
    <div onContextMenu={handleContextMenu} onDragOver={onDragOver} onDragEnter={onDragEnter} onDragLeave={onDragLeave} onDrop={onDrop}>
      {children}

      {menuPos && (
        <div
          ref={menuRef}
          className={cn(
            "fixed z-50 min-w-[160px] rounded-md border border-border bg-popover py-1 shadow-md",
          )}
          style={{ left: menuPos.x, top: menuPos.y }}
        >
          <button
            type="button"
            onClick={handleRename}
            className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-accent"
          >
            <Pencil className="size-3.5" />
            Rename
          </button>
          <button
            type="button"
            onClick={handleDelete}
            disabled={deleteFolder.isPending}
            className={cn(
              "flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-accent",
              confirmDelete
                ? "text-destructive font-medium"
                : "text-foreground",
            )}
          >
            <Trash2 className="size-3.5" />
            {deleteFolder.isPending
              ? "Deleting..."
              : confirmDelete
                ? "Confirm delete?"
                : "Delete"}
          </button>
        </div>
      )}
    </div>
  );
}
