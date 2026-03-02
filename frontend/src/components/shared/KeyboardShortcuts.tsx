"use client";

import { Dialog } from "radix-ui";
import { useUiStore } from "@/stores/useUiStore";

const shortcuts = [
  { keys: ["⌘", "K"], description: "Search" },
  { keys: ["⌘", "P"], description: "Command palette" },
  { keys: ["J", "/", "↓"], description: "Next message" },
  { keys: ["K", "/", "↑"], description: "Previous message" },
  { keys: ["S"], description: "Toggle star" },
  { keys: ["U"], description: "Toggle read/unread" },
  { keys: ["Delete"], description: "Delete / move to trash" },
  { keys: ["Escape"], description: "Close panel / clear search" },
  { keys: ["?"], description: "Show this reference" },
];

export function KeyboardShortcuts() {
  const open = useUiStore((s) => s.shortcutsOpen);
  const setOpen = useUiStore((s) => s.setShortcutsOpen);

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/40" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl border border-border bg-background p-6 shadow-2xl">
          <Dialog.Title className="mb-4 text-lg font-semibold">
            Keyboard Shortcuts
          </Dialog.Title>
          <div className="space-y-2">
            {shortcuts.map((s) => (
              <div
                key={s.description}
                className="flex items-center justify-between py-1.5"
              >
                <span className="text-sm text-foreground">{s.description}</span>
                <div className="flex items-center gap-1">
                  {s.keys.map((key, i) => (
                    <span key={i}>
                      {key === "/" ? (
                        <span className="px-1 text-xs text-muted-foreground">/</span>
                      ) : (
                        <kbd className="inline-flex min-w-[24px] items-center justify-center rounded-md border border-border bg-muted px-1.5 py-0.5 text-xs font-medium text-muted-foreground">
                          {key}
                        </kbd>
                      )}
                    </span>
                  ))}
                </div>
              </div>
            ))}
          </div>
          <Dialog.Close asChild>
            <button className="mt-4 w-full rounded-lg bg-accent py-2 text-sm text-accent-foreground hover:bg-accent/80">
              Close
            </button>
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
