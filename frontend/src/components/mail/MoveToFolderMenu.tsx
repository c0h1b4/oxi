"use client";

import { Popover } from "radix-ui";

import { useState, useMemo } from "react";
import { FolderInput } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useFolders } from "@/hooks/useFolders";
import { formatFolderName } from "./FolderTree";
import { useUiStore } from "@/stores/useUiStore";
import { createScaleFadeVariants } from "@/lib/motion/variants";
import { AnimatedDiv } from "@/lib/motion/AnimatedDiv";

interface MoveToFolderMenuProps {
  currentFolder: string;
  onMove: (toFolder: string) => void;
}

export function MoveToFolderMenu({
  currentFolder,
  onMove,
}: MoveToFolderMenuProps) {
  const [open, setOpen] = useState(false);
  const { data } = useFolders();
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const menuMotionProps = useMemo(() => createScaleFadeVariants(effectiveAnimationMode), [effectiveAnimationMode]);

  const folders =
    data?.folders.filter((f) => f.name !== currentFolder) ?? [];

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <Button variant="ghost" size="sm" className="gap-1.5">
          <FolderInput className="size-4" />
          Move to...
        </Button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content sideOffset={4} collisionPadding={8} className="z-50 p-0 outline-none">
          <AnimatedDiv
            data-testid="move-to-folder-menu-transition"
            variants={menuMotionProps}
            initial="initial"
            animate="animate"
            exit="exit"
            className="max-h-[var(--radix-popover-content-available-height)] w-48 max-w-[calc(100vw-1rem)] overflow-y-auto rounded-md border border-border bg-popover py-1 shadow-md [overflow-wrap:anywhere]"
          >
            {folders.length === 0 ? (
              <div className="px-3 py-2 text-sm text-muted-foreground">No other folders</div>
            ) : folders.map((folder) => (
              <button
                key={folder.name}
                type="button"
                onClick={() => {
                  onMove(folder.name);
                  setOpen(false);
                }}
                className="flex w-full items-center px-3 py-1.5 text-left text-sm transition-colors hover:bg-accent"
              >
                {formatFolderName(folder.name)}
              </button>
            ))}
          </AnimatedDiv>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
