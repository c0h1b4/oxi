"use client";

import { Command } from "cmdk";
import { Dialog } from "radix-ui";
import { AnimatePresence, motion } from "framer-motion";
import {
  PenSquare,
  Inbox,
  Send,
  FileText,
  Trash2,
  Search,
  Moon,
  Sun,
  Settings,
  Users,
  Keyboard,
} from "lucide-react";
import { useUiStore } from "@/stores/useUiStore";
import { useComposeStore } from "@/stores/useComposeStore";
import { useUpdateDisplayPreferences } from "@/hooks/useDisplayPreferences";
import { runThemeSpreadTransition } from "@/lib/motion/theme-spread";
import { createFadeSlideVariants, createScaleFadeVariants } from "@/lib/motion/variants";

function useResolvedTheme() {
  const theme = useUiStore((s) => s.theme);
  if (theme === "system") {
    if (typeof window === "undefined") return "light";
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return theme;
}

const THEME_STORAGE_KEY = "oxi-theme";

export function CommandPalette() {
  const open = useUiStore((s) => s.commandPaletteOpen);
  const setOpen = useUiStore((s) => s.setCommandPaletteOpen);
  const setTheme = useUiStore((s) => s.setTheme);
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const shouldAnimate = effectiveAnimationMode !== "off";
  const overlayMotionProps = createFadeSlideVariants(effectiveAnimationMode, "y");
  const contentMotionProps = createScaleFadeVariants(effectiveAnimationMode);
  const PaletteContainer = shouldAnimate ? motion.div : "div";
  const updatePrefs = useUpdateDisplayPreferences();
  const resolvedTheme = useResolvedTheme();

  const runAndClose = (fn: () => void) => {
    fn();
    setOpen(false);
  };

  const toggleTheme = () => {
    const next = resolvedTheme === "dark" ? "light" : "dark";
    runThemeSpreadTransition({
      mode: effectiveAnimationMode,
      trigger: "explicit",
      applyTheme: () => setTheme(next),
      nextTheme: next,
    });
    localStorage.setItem(THEME_STORAGE_KEY, next);
    updatePrefs.mutate({ theme: next });
  };

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Portal>
        <AnimatePresence>
          {open ? (
            <>
              <Dialog.Overlay asChild={shouldAnimate}>
                {shouldAnimate ? (
                  <motion.div
                    key="command-palette-overlay"
                    data-testid="command-palette-overlay-transition"
                    data-motion-props={JSON.stringify(overlayMotionProps)}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    variants={overlayMotionProps}
                    className="fixed inset-0 z-50 bg-black/40"
                  />
                ) : (
                  <div className="fixed inset-0 z-50 bg-black/40" />
                )}
              </Dialog.Overlay>
              <Dialog.Content
                asChild={shouldAnimate}
                className={
                  shouldAnimate
                    ? undefined
                    : "fixed left-1/2 top-[20%] z-50 w-full max-w-lg -translate-x-1/2 overflow-hidden rounded-xl border border-border bg-background shadow-2xl"
                }
              >
                <PaletteContainer
                  {...(shouldAnimate
                    ? {
                        "data-testid": "command-palette-content-transition",
                        "data-motion-props": JSON.stringify(contentMotionProps),
                        initial: "initial",
                        animate: "animate",
                        exit: "exit",
                        variants: contentMotionProps,
                        className:
                          "fixed left-1/2 top-[20%] z-50 w-full max-w-lg -translate-x-1/2 overflow-hidden rounded-xl border border-border bg-background shadow-2xl",
                      }
                    : {})}
                >
                  <Command className="flex flex-col" label="Command palette">
            <div className="flex items-center border-b border-border px-3">
              <Search className="mr-2 size-4 shrink-0 text-muted-foreground" />
              <Command.Input
                placeholder="Type a command..."
                className="flex-1 bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground/50"
              />
            </div>
            <Command.List className="max-h-72 overflow-y-auto p-2">
              <Command.Empty className="px-3 py-6 text-center text-sm text-muted-foreground">
                No results found.
              </Command.Empty>

              <Command.Group heading="Actions" className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-xs [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-muted-foreground">
                <CommandItem
                  icon={<PenSquare className="size-4" />}
                  onSelect={() => runAndClose(() => useComposeStore.getState().openCompose())}
                >
                  Compose new email
                </CommandItem>
                <CommandItem
                  icon={<Search className="size-4" />}
                  onSelect={() =>
                    runAndClose(() => {
                      setTimeout(() => {
                        (document.querySelector("[data-search-input]") as HTMLElement)?.focus();
                      }, 0);
                    })
                  }
                >
                  Search emails
                </CommandItem>
                <CommandItem
                  icon={resolvedTheme === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
                  onSelect={() => runAndClose(toggleTheme)}
                >
                  Toggle dark mode
                </CommandItem>
                <CommandItem
                  icon={<Keyboard className="size-4" />}
                  onSelect={() =>
                    runAndClose(() => useUiStore.getState().setShortcutsOpen(true))
                  }
                >
                  Keyboard shortcuts
                </CommandItem>
              </Command.Group>

              <Command.Group heading="Navigate" className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-xs [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-muted-foreground">
                <CommandItem
                  icon={<Inbox className="size-4" />}
                  onSelect={() =>
                    runAndClose(() => {
                      useUiStore.getState().setViewMode("mail");
                      useUiStore.getState().setActiveFolder("INBOX");
                    })
                  }
                >
                  Go to Inbox
                </CommandItem>
                <CommandItem
                  icon={<Send className="size-4" />}
                  onSelect={() =>
                    runAndClose(() => {
                      useUiStore.getState().setViewMode("mail");
                      useUiStore.getState().setActiveFolder("Sent");
                    })
                  }
                >
                  Go to Sent
                </CommandItem>
                <CommandItem
                  icon={<FileText className="size-4" />}
                  onSelect={() =>
                    runAndClose(() => {
                      useUiStore.getState().setViewMode("mail");
                      useUiStore.getState().setActiveFolder("Drafts");
                    })
                  }
                >
                  Go to Drafts
                </CommandItem>
                <CommandItem
                  icon={<Trash2 className="size-4" />}
                  onSelect={() =>
                    runAndClose(() => {
                      useUiStore.getState().setViewMode("mail");
                      useUiStore.getState().setActiveFolder("Trash");
                    })
                  }
                >
                  Go to Trash
                </CommandItem>
                <CommandItem
                  icon={<Settings className="size-4" />}
                  onSelect={() =>
                    runAndClose(() => useUiStore.getState().setViewMode("settings"))
                  }
                >
                  Open Settings
                </CommandItem>
                <CommandItem
                  icon={<Users className="size-4" />}
                  onSelect={() =>
                    runAndClose(() => useUiStore.getState().setViewMode("contacts"))
                  }
                >
                  Open Contacts
                </CommandItem>
              </Command.Group>
            </Command.List>
                  </Command>
                </PaletteContainer>
              </Dialog.Content>
            </>
          ) : null}
        </AnimatePresence>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function CommandItem({
  children,
  icon,
  onSelect,
}: {
  children: React.ReactNode;
  icon: React.ReactNode;
  onSelect: () => void;
}) {
  return (
    <Command.Item
      onSelect={onSelect}
      className="flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2 text-sm text-foreground aria-selected:bg-accent aria-selected:text-accent-foreground"
    >
      <span className="text-muted-foreground">{icon}</span>
      {children}
    </Command.Item>
  );
}
