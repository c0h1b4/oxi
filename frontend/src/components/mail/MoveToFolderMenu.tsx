"use client";

import { useState, useRef, useEffect } from "react";
import { FolderInput } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useFolders } from "@/hooks/useFolders";

interface MoveToFolderMenuProps {
  currentFolder: string;
  onMove: (toFolder: string) => void;
}

export function MoveToFolderMenu({
  currentFolder,
  onMove,
}: MoveToFolderMenuProps) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const { data } = useFolders();

  const folders =
    data?.folders.filter((f) => f.name !== currentFolder) ?? [];

  // Close dropdown when clicking outside
  useEffect(() => {
    if (!open) return;
    function handleClick(e: MouseEvent) {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [open]);

  return (
    <div ref={containerRef} className="relative">
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setOpen((prev) => !prev)}
        className="gap-1.5"
      >
        <FolderInput className="size-4" />
        Move to...
      </Button>

      {open && (
        <div className="absolute left-0 top-full z-50 mt-1 min-w-[180px] rounded-md border border-border bg-popover py-1 shadow-md">
          {folders.length === 0 ? (
            <div className="px-3 py-2 text-sm text-muted-foreground">
              No other folders
            </div>
          ) : (
            folders.map((folder) => (
              <button
                key={folder.name}
                type="button"
                onClick={() => {
                  onMove(folder.name);
                  setOpen(false);
                }}
                className="flex w-full items-center px-3 py-1.5 text-left text-sm transition-colors hover:bg-accent"
              >
                {folder.name}
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
}
