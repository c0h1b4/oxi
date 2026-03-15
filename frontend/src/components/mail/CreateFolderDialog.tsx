"use client";

import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import { Dialog } from "radix-ui";
import { AnimatePresence } from "framer-motion";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useCreateFolder } from "@/hooks/useFolders";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/useUiStore";
import { createFadeSlideVariants, createScaleFadeVariants } from "@/lib/motion/variants";
import { AnimatedDiv } from "@/lib/motion/AnimatedDiv";

interface CreateFolderDialogProps {
  open: boolean;
  onClose: () => void;
}

export function CreateFolderDialog({ open, onClose }: CreateFolderDialogProps) {
  const [name, setName] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const createFolder = useCreateFolder();
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const overlayMotionProps = useMemo(() => createFadeSlideVariants(effectiveAnimationMode, "y"), [effectiveAnimationMode]);
  const contentMotionProps = useMemo(() => createScaleFadeVariants(effectiveAnimationMode), [effectiveAnimationMode]);

  // Reset state and focus input when dialog opens.
  useEffect(() => {
    if (open) {
      setName(""); // eslint-disable-line react-hooks/set-state-in-effect -- intentional reset on dialog open
      createFolder.reset();
      // Small delay to ensure the DOM has rendered
      const timer = setTimeout(() => inputRef.current?.focus(), 50);
      return () => clearTimeout(timer);
    }
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const trimmed = name.trim();
      if (!trimmed) return;

      createFolder.mutate(
        { name: trimmed },
        {
          onSuccess: () => onClose(),
        },
      );
    },
    [name, createFolder, onClose],
  );

  return (
    <Dialog.Root open={open} onOpenChange={(o) => !o && onClose()}>
      <Dialog.Portal forceMount>
        <AnimatePresence>
          {open ? (
            <Dialog.Overlay asChild forceMount>
              <AnimatedDiv
                key="create-folder-overlay"
                data-testid="create-folder-overlay-transition"
                variants={overlayMotionProps}
                initial="initial"
                animate="animate"
                exit="exit"
                className="fixed inset-0 z-50 bg-black/50"
              />
            </Dialog.Overlay>
          ) : null}
          {open ? (
            <Dialog.Content asChild forceMount>
              <AnimatedDiv
                key="create-folder-content"
                data-testid="create-folder-content-transition"
                variants={contentMotionProps}
                initial="initial"
                animate="animate"
                exit="exit"
                className="fixed left-1/2 top-1/2 z-50 w-full max-w-sm -translate-x-1/2 -translate-y-1/2 rounded-lg border border-border bg-popover p-6 shadow-lg"
              >
                <Dialog.Title className="mb-4 text-base font-semibold text-foreground">
                  Create new folder
                </Dialog.Title>

                <form onSubmit={handleSubmit}>
                  <input
                    ref={inputRef}
                    type="text"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    placeholder="Folder name"
                    className={cn(
                      "w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground",
                      "placeholder:text-muted-foreground",
                      "outline-none focus:border-ring focus:ring-2 focus:ring-ring/50",
                    )}
                    disabled={createFolder.isPending}
                    autoComplete="off"
                  />

                  {createFolder.isError && (
                    <p className="mt-2 text-sm text-destructive">
                      {createFolder.error?.message ?? "Failed to create folder"}
                    </p>
                  )}

                  <div className="mt-4 flex justify-end gap-2">
                    <Dialog.Close asChild>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        disabled={createFolder.isPending}
                      >
                        Cancel
                      </Button>
                    </Dialog.Close>
                    <Button
                      type="submit"
                      size="sm"
                      disabled={!name.trim() || createFolder.isPending}
                    >
                      {createFolder.isPending && (
                        <Loader2 className="size-3.5 animate-spin" />
                      )}
                      Create
                    </Button>
                  </div>
                </form>
              </AnimatedDiv>
            </Dialog.Content>
          ) : null}
        </AnimatePresence>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
