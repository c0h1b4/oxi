"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { Dialog } from "radix-ui";
import { ArrowLeft, Menu, X } from "lucide-react";
import { useMediaQuery } from "@/hooks/useMediaQuery";
import { Button } from "@/components/ui/button";
import { useUiStore } from "@/stores/useUiStore";
import { AnimatedDiv } from "@/lib/motion/AnimatedDiv";
import { SearchBar } from "@/components/mail/SearchBar";
import { SearchResults } from "@/components/mail/SearchResults";
import { MessageActionBar } from "@/components/mail/MessageActionBar";
import { isValidCommittedSearch } from "@/lib/search-parser";

interface ThreePanelLayoutProps {
  sidebar: React.ReactNode;
  messageList: React.ReactNode;
  readingPane: React.ReactNode;
}

const centerTransition = {
  initial: { opacity: 0, x: 8 },
  animate: {
    opacity: 1,
    x: 0,
    transition: { duration: 0.22, ease: [0.2, 0, 0, 1] as const },
  },
  exit: {
    opacity: 0,
    x: -4,
    transition: { duration: 0.14, ease: [0.2, 0, 0, 1] as const },
  },
};

const readingPaneTransition = {
  initial: { opacity: 0, x: 12 },
  animate: {
    opacity: 1,
    x: 0,
    transition: { duration: 0.22, ease: [0.2, 0, 0, 1] as const },
  },
  exit: {
    opacity: 0,
    x: 6,
    transition: { duration: 0.14, ease: [0.2, 0, 0, 1] as const },
  },
};

const MIN_SIDEBAR_WIDTH = 140;
const MAX_SIDEBAR_WIDTH = 400;
const MIN_MESSAGE_LIST_WIDTH = 280;
const MAX_MESSAGE_LIST_WIDTH = 700;
const MIN_READING_PANE_WIDTH = 420;
const RESIZE_HANDLE_BUDGET = 8;

function ResizeHandle({
  onDrag,
}: {
  onDrag: (deltaX: number) => void;
}) {
  const dragging = useRef(false);
  const lastX = useRef(0);

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      dragging.current = true;
      lastX.current = e.clientX;

      const onMouseMove = (ev: MouseEvent) => {
        if (!dragging.current) return;
        const delta = ev.clientX - lastX.current;
        lastX.current = ev.clientX;
        onDrag(delta);
      };

      const onMouseUp = () => {
        dragging.current = false;
        document.removeEventListener("mousemove", onMouseMove);
        document.removeEventListener("mouseup", onMouseUp);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
      };

      document.addEventListener("mousemove", onMouseMove);
      document.addEventListener("mouseup", onMouseUp);
      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
    },
    [onDrag],
  );

  return (
    <button
      type="button"
      aria-label="Resize panel"
      onMouseDown={onMouseDown}
      className="group relative z-10 w-0 cursor-col-resize outline-none"
    >
      {/* Invisible wider hit area */}
      <div className="absolute inset-y-0 -left-1 w-2 group-hover:bg-primary/20 group-active:bg-primary/30" />
    </button>
  );
}

export function ThreePanelLayout({
  sidebar,
  messageList,
  readingPane,
}: ThreePanelLayoutProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLElement>(null);
  const splitView = useMediaQuery("(min-width: 1024px)");
  const persistentSidebar = useMediaQuery("(min-width: 1280px)");
  const sidebarOpen = useUiStore((s) => s.sidebarOpen);
  const setSidebarOpen = useUiStore((s) => s.setSidebarOpen);
  const selectMessage = useUiStore((s) => s.selectMessage);
  const sidebarWidth = useUiStore((s) => s.sidebarWidth);
  const messageListWidth = useUiStore((s) => s.messageListWidth);
  const setSidebarWidth = useUiStore((s) => s.setSidebarWidth);
  const setMessageListWidth = useUiStore((s) => s.setMessageListWidth);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const searchActive = useUiStore((s) => s.searchActive);
  const searchQuery = useUiStore((s) => s.searchQuery);
  const readingPaneVisible = useUiStore((s) => s.readingPaneVisible);
  const hasValidCommittedSearch = isValidCommittedSearch(searchQuery);
  const showSearchResults = searchActive && hasValidCommittedSearch;
  const [containerWidth, setContainerWidth] = useState<number | null>(null);
  const showReadingPane = splitView ? readingPaneVisible : selectedMessageUid !== null;
  const showMessageList = splitView || selectedMessageUid === null;
  const effectiveSidebarWidth = persistentSidebar ? sidebarWidth : 0;

  useEffect(() => {
    if (persistentSidebar) setSidebarOpen(false);
  }, [persistentSidebar, setSidebarOpen]);

  useEffect(() => {
    const node = containerRef.current;
    if (!node) return;

    setContainerWidth(node.getBoundingClientRect().width);
    if (typeof ResizeObserver === "undefined") return;

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      setContainerWidth(entry.contentRect.width);
    });

    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  const maxMessageListWidth = useMemo(() => {
    if (!readingPaneVisible) {
      return MAX_MESSAGE_LIST_WIDTH;
    }

    if (containerWidth == null) {
      return MAX_MESSAGE_LIST_WIDTH;
    }

    const remaining = Math.floor(
      containerWidth -
      effectiveSidebarWidth -
      RESIZE_HANDLE_BUDGET -
      MIN_READING_PANE_WIDTH,
    );

    return Math.max(
      MIN_MESSAGE_LIST_WIDTH,
      Math.min(MAX_MESSAGE_LIST_WIDTH, remaining),
    );
  }, [containerWidth, readingPaneVisible, effectiveSidebarWidth]);

  const resolvedMessageListWidth = readingPaneVisible
    ? Math.max(MIN_MESSAGE_LIST_WIDTH, Math.min(messageListWidth, maxMessageListWidth))
    : null;

  const handleSidebarDrag = useCallback(
    (delta: number) => {
      const current = useUiStore.getState().sidebarWidth;
      setSidebarWidth(Math.max(MIN_SIDEBAR_WIDTH, Math.min(MAX_SIDEBAR_WIDTH, current + delta)));
    },
    [setSidebarWidth],
  );

  const handleMessageListDrag = useCallback(
    (delta: number) => {
      const current = useUiStore.getState().messageListWidth;
      setMessageListWidth(
        Math.max(MIN_MESSAGE_LIST_WIDTH, Math.min(maxMessageListWidth, current + delta)),
      );
    },
    [maxMessageListWidth, setMessageListWidth],
  );

  return (
    <div ref={containerRef} className="relative flex h-full min-h-0 w-full overflow-hidden">
      {persistentSidebar ? (
        <>
          <aside aria-label="Folders" className="shrink-0 overflow-y-auto bg-sidebar" style={{ width: sidebarWidth }}>
            {sidebar}
          </aside>
          <ResizeHandle onDrag={handleSidebarDrag} />
        </>
      ) : (
        <Dialog.Root open={sidebarOpen} onOpenChange={setSidebarOpen}>
          <Dialog.Trigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="absolute left-2 top-1 z-10"
              aria-label="Open folders"
            >
              <Menu className="size-5" />
            </Button>
          </Dialog.Trigger>
          <Dialog.Portal>
            <Dialog.Overlay className="fixed inset-0 z-40 bg-black/40" />
            <Dialog.Content className="fixed inset-y-0 left-0 z-50 flex w-80 max-w-[calc(100%-2rem)] flex-col bg-sidebar shadow-xl">
              <div className="flex shrink-0 items-center justify-between border-b border-border px-4 py-2">
                <Dialog.Title className="font-semibold">Folders</Dialog.Title>
                <Dialog.Description className="sr-only">Choose an account, folder, or tag.</Dialog.Description>
                <Dialog.Close asChild>
                  <Button variant="ghost" size="icon" aria-label="Close folders"><X className="size-5" /></Button>
                </Dialog.Close>
              </div>
              <div className="min-h-0 flex-1">{sidebar}</div>
            </Dialog.Content>
          </Dialog.Portal>
        </Dialog.Root>
      )}

      {/* Center panel — search bar + message list or search results */}
      <main
        ref={listRef}
        tabIndex={-1}
        aria-label="Messages"
        hidden={!showMessageList}
        className={!showMessageList ? "hidden" : splitView && readingPaneVisible
          ? "flex min-w-0 shrink-0 flex-col overflow-hidden border-x border-border"
          : "flex min-w-0 flex-1 flex-col overflow-hidden border-l border-border"}
        style={splitView && readingPaneVisible ? { width: resolvedMessageListWidth ?? messageListWidth } : undefined}
      >
        <div className={persistentSidebar ? "shrink-0" : "shrink-0 pl-12"}><SearchBar /></div>
        <AnimatePresence mode="wait" initial={false}>
          {showSearchResults ? (
            <AnimatedDiv
              key="search"
              data-testid="three-panel-search-transition"
              variants={centerTransition}
              initial={centerTransition.initial}
              animate={centerTransition.animate}
              exit={centerTransition.exit}
              className="flex min-h-0 flex-1 flex-col"
            >
              <SearchResults />
            </AnimatedDiv>
          ) : (
            <AnimatedDiv
              key="list"
              data-testid="three-panel-list-transition"
              variants={centerTransition}
              initial={centerTransition.initial}
              animate={centerTransition.animate}
              exit={centerTransition.exit}
              className="min-h-0 flex-1 overflow-y-auto"
            >
              {messageList}
            </AnimatedDiv>
          )}
        </AnimatePresence>
      </main>

      {/* Resize handle: message list | reading pane */}
      <AnimatePresence initial={false}>
        {showReadingPane && (
          <AnimatedDiv
            key="reading-pane"
            data-testid="three-panel-reading-pane-transition"
            variants={readingPaneTransition}
            initial={readingPaneTransition.initial}
            animate={readingPaneTransition.animate}
            exit={readingPaneTransition.exit}
            className="flex min-h-0 min-w-0 flex-1"
          >
            {splitView && <ResizeHandle onDrag={handleMessageListDrag} />}
            <section className="flex min-h-0 min-w-0 flex-1 flex-col">
              {!splitView && (
                <div className="flex h-12 shrink-0 items-center border-b border-border pl-14">
                  <Button variant="ghost" onClick={() => {
                    selectMessage(null);
                    requestAnimationFrame(() => listRef.current?.focus());
                  }}>
                    <ArrowLeft className="size-4" />
                    {showSearchResults ? "Back to results" : "Back to messages"}
                  </Button>
                </div>
              )}
              <MessageActionBar />
              {selectedMessageUid !== null ? (
                <div className="flex min-h-0 min-w-0 flex-1">{readingPane}</div>
              ) : (
                <div className="flex h-full w-full items-center justify-center">
                  <span className="text-2xl font-bold tracking-tight text-muted-foreground/40">
                    oxi<span className="text-primary/40">.email</span>
                  </span>
                </div>
              )}
            </section>
          </AnimatedDiv>
        )}
      </AnimatePresence>
    </div>
  );
}
