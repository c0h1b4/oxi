"use client";

import { useState, useRef, useEffect, useCallback } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useCreateFolder } from "@/hooks/useFolders";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/useUiStore";
import { createFadeSlideVariants, createScaleFadeVariants } from "@/lib/motion/variants";

interface CreateFolderDialogProps {
  open: boolean;
  onClose: () => void;
}

export function CreateFolderDialog({ open, onClose }: CreateFolderDialogProps) {
  const [name, setName] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const createFolder = useCreateFolder();
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const shouldAnimate = effectiveAnimationMode !== "off";
  const overlayMotionProps = createFadeSlideVariants(effectiveAnimationMode, "y");
  const contentMotionProps = createScaleFadeVariants(effectiveAnimationMode);

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

  // Close on Escape
  useEffect(() => {
    if (!open) return;
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        onClose();
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  if (!shouldAnimate && !open) return null;

  if (shouldAnimate) {
    return (
      <AnimatePresence>
        {open ? (
          <div
            key="create-folder-dialog"
            className="fixed inset-0 z-50 flex items-center justify-center"
            role="dialog"
            aria-modal="true"
            aria-label="Create new folder"
          >
            <motion.div
              key="create-folder-overlay"
              data-testid="create-folder-overlay-transition"
              data-motion-props={JSON.stringify(overlayMotionProps)}
              initial="initial"
              animate="animate"
              exit="exit"
              variants={overlayMotionProps}
              className="absolute inset-0 bg-black/50"
              onClick={onClose}
            />

            <motion.div
              key="create-folder-content"
              data-testid="create-folder-content-transition"
              data-motion-props={JSON.stringify(contentMotionProps)}
              initial="initial"
              animate="animate"
              exit="exit"
              variants={contentMotionProps}
              className="relative z-10 w-full max-w-sm rounded-lg border border-border bg-popover p-6 shadow-lg"
            >
              <h2 className="mb-4 text-base font-semibold text-foreground">
                Create new folder
              </h2>

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
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={onClose}
                    disabled={createFolder.isPending}
                  >
                    Cancel
                  </Button>
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
            </motion.div>
          </div>
        ) : null}
      </AnimatePresence>
    );
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      role="dialog"
      aria-modal="true"
      aria-label="Create new folder"
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50"
        onClick={onClose}
      />

      {/* Dialog content */}
      <div className="relative z-10 w-full max-w-sm rounded-lg border border-border bg-popover p-6 shadow-lg">
        <h2 className="mb-4 text-base font-semibold text-foreground">
          Create new folder
        </h2>

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
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onClose}
              disabled={createFolder.isPending}
            >
              Cancel
            </Button>
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
      </div>
    </div>
  );
}
