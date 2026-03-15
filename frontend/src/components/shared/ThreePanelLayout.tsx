"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { useUiStore } from "@/stores/useUiStore";
import { SearchBar } from "@/components/mail/SearchBar";
import { SearchResults } from "@/components/mail/SearchResults";
import { MessageActionBar } from "@/components/mail/MessageActionBar";

interface ThreePanelLayoutProps {
  navRail: React.ReactNode;
  sidebar: React.ReactNode;
  messageList: React.ReactNode;
  readingPane: React.ReactNode;
}

const MIN_SIDEBAR_WIDTH = 140;
const MAX_SIDEBAR_WIDTH = 400;
const MIN_MESSAGE_LIST_WIDTH = 280;
const MAX_MESSAGE_LIST_WIDTH = 700;
const MIN_READING_PANE_WIDTH = 420;
const NAV_RAIL_WIDTH = 56;
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
  navRail,
  sidebar,
  messageList,
  readingPane,
}: ThreePanelLayoutProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sidebarWidth = useUiStore((s) => s.sidebarWidth);
  const messageListWidth = useUiStore((s) => s.messageListWidth);
  const setSidebarWidth = useUiStore((s) => s.setSidebarWidth);
  const setMessageListWidth = useUiStore((s) => s.setMessageListWidth);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const searchActive = useUiStore((s) => s.searchActive);
  const readingPaneVisible = useUiStore((s) => s.readingPaneVisible);
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const shouldAnimate = effectiveAnimationMode !== "off";
  const [containerWidth, setContainerWidth] = useState<number | null>(null);

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
      sidebarWidth -
      NAV_RAIL_WIDTH -
      RESIZE_HANDLE_BUDGET -
      MIN_READING_PANE_WIDTH,
    );

    return Math.max(
      MIN_MESSAGE_LIST_WIDTH,
      Math.min(MAX_MESSAGE_LIST_WIDTH, remaining),
    );
  }, [containerWidth, readingPaneVisible, sidebarWidth]);

  const resolvedMessageListWidth = readingPaneVisible
    ? Math.max(MIN_MESSAGE_LIST_WIDTH, Math.min(messageListWidth, maxMessageListWidth))
    : null;

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
    <div ref={containerRef} className="flex h-full min-h-0 w-full overflow-hidden">
      {/* Navigation rail */}
      {navRail}

      {/* Folder sidebar */}
      <aside
        className="shrink-0 overflow-y-auto bg-sidebar"
        style={{ width: sidebarWidth }}
      >
        {sidebar}
      </aside>

      {/* Resize handle: sidebar | message list */}
      <ResizeHandle onDrag={handleSidebarDrag} />

      {/* Center panel — search bar + message list or search results */}
      <main
        className={
          readingPaneVisible
            ? "flex shrink-0 flex-col overflow-hidden border-x border-border"
            : "flex min-w-0 flex-1 flex-col overflow-hidden border-l border-border"
        }
        style={readingPaneVisible ? { width: resolvedMessageListWidth ?? messageListWidth } : undefined}
      >
        <SearchBar />
        {shouldAnimate ? (
          <AnimatePresence mode="wait" initial={false}>
            {searchActive ? (
              <motion.div
                key="search"
                data-testid="three-panel-search-transition"
                data-motion-props={JSON.stringify(centerTransition)}
                initial={centerTransition.initial}
                animate={centerTransition.animate}
                exit={centerTransition.exit}
                className="min-h-0 flex-1"
              >
                <SearchResults />
              </motion.div>
            ) : (
              <motion.div
                key="list"
                data-testid="three-panel-list-transition"
                data-motion-props={JSON.stringify(centerTransition)}
                initial={centerTransition.initial}
                animate={centerTransition.animate}
                exit={centerTransition.exit}
                className="flex-1 overflow-y-auto"
              >
                {messageList}
              </motion.div>
            )}
          </AnimatePresence>
        ) : searchActive ? (
          <SearchResults />
        ) : (
          <div className="flex-1 overflow-y-auto">{messageList}</div>
        )}
      </main>

      {/* Resize handle: message list | reading pane */}
      {shouldAnimate ? (
        <AnimatePresence initial={false}>
          {readingPaneVisible && (
            <motion.div
              key="reading-pane"
              data-testid="three-panel-reading-pane-transition"
              data-motion-props={JSON.stringify(readingPaneTransition)}
              initial={readingPaneTransition.initial}
              animate={readingPaneTransition.animate}
              exit={readingPaneTransition.exit}
              className="flex min-h-0 min-w-0 flex-1"
            >
              <ResizeHandle onDrag={handleMessageListDrag} />
              <section className="flex min-h-0 min-w-0 flex-1 flex-col">
                <MessageActionBar />
                {selectedMessageUid !== null ? (
                  <div className="flex min-h-0 flex-1">{readingPane}</div>
                ) : (
                  <div className="flex h-full w-full items-center justify-center">
                    <span className="text-2xl font-bold tracking-tight text-muted-foreground/40">
                      oxi<span className="text-primary/40">.email</span>
                    </span>
                  </div>
                )}
              </section>
            </motion.div>
          )}
        </AnimatePresence>
      ) : (
        <>
          {readingPaneVisible && <ResizeHandle onDrag={handleMessageListDrag} />}
          {readingPaneVisible && (
            <section className="flex min-h-0 min-w-0 flex-1 flex-col">
              <MessageActionBar />
              {selectedMessageUid !== null ? (
                <div className="flex min-h-0 flex-1">{readingPane}</div>
              ) : (
                <div className="flex h-full w-full items-center justify-center">
                  <span className="text-2xl font-bold tracking-tight text-muted-foreground/40">
                    oxi<span className="text-primary/40">.email</span>
                  </span>
                </div>
              )}
            </section>
          )}
        </>
      )}
    </div>
  );
}
