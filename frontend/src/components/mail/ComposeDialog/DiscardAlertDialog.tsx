"use client";

import { useMemo } from "react";
import { AlertDialog } from "radix-ui";
import { AnimatePresence } from "framer-motion";
import { createFadeSlideVariants, createScaleFadeVariants } from "@/lib/motion/variants";
import { AnimatedDiv } from "@/lib/motion/AnimatedDiv";
import { useUiStore } from "@/stores/useUiStore";

interface DiscardAlertDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}

export function DiscardAlertDialog({
  open,
  onOpenChange,
  onConfirm,
}: DiscardAlertDialogProps) {
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const overlayMotionProps = useMemo(() => createFadeSlideVariants(effectiveAnimationMode, "y"), [effectiveAnimationMode]);
  const contentMotionProps = useMemo(() => createScaleFadeVariants(effectiveAnimationMode), [effectiveAnimationMode]);

  return (
    <AlertDialog.Root open={open} onOpenChange={onOpenChange}>
      <AlertDialog.Portal>
        <AnimatePresence>
          {open ? (
            <>
              <AlertDialog.Overlay asChild>
                <AnimatedDiv
                  data-testid="discard-alert-overlay-transition"
                  variants={overlayMotionProps}
                  initial="initial"
                  animate="animate"
                  exit="exit"
                  className="fixed inset-0 z-50 bg-black/40"
                />
              </AlertDialog.Overlay>
              <AlertDialog.Content asChild>
                <AnimatedDiv
                  data-testid="discard-alert-content-transition"
                  variants={contentMotionProps}
                  initial="initial"
                  animate="animate"
                  exit="exit"
                  className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl border border-border bg-background p-6 shadow-2xl"
                >
                  <AlertDialog.Title className="text-base font-semibold">
                    Are you sure?
                  </AlertDialog.Title>
                  <AlertDialog.Description className="mt-2 text-sm text-muted-foreground">
                    The message has not been sent and has unsaved changes. Do you want
                    to discard your changes?
                  </AlertDialog.Description>
                  <div className="mt-6 flex justify-end gap-3">
                    <AlertDialog.Cancel asChild>
                      <button className="rounded-lg px-4 py-2 text-sm font-medium text-muted-foreground hover:bg-accent hover:text-foreground">
                        Cancel
                      </button>
                    </AlertDialog.Cancel>
                    <AlertDialog.Action asChild>
                      <button
                        onClick={onConfirm}
                        className="rounded-lg bg-destructive px-4 py-2 text-sm font-medium text-destructive-foreground hover:bg-destructive/90"
                      >
                        Discard
                      </button>
                    </AlertDialog.Action>
                  </div>
                </AnimatedDiv>
              </AlertDialog.Content>
            </>
          ) : null}
        </AnimatePresence>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
