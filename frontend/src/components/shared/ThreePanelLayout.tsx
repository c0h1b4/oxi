"use client";

import { useCallback, useRef } from "react";
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
    <div
      onMouseDown={onMouseDown}
      className="group relative z-10 w-0 cursor-col-resize"
    >
      {/* Invisible wider hit area */}
      <div className="absolute inset-y-0 -left-1 w-2 group-hover:bg-primary/20 group-active:bg-primary/30" />
    </div>
  );
}

export function ThreePanelLayout({
  navRail,
  sidebar,
  messageList,
  readingPane,
}: ThreePanelLayoutProps) {
  const sidebarWidth = useUiStore((s) => s.sidebarWidth);
  const messageListWidth = useUiStore((s) => s.messageListWidth);
  const setSidebarWidth = useUiStore((s) => s.setSidebarWidth);
  const setMessageListWidth = useUiStore((s) => s.setMessageListWidth);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const searchActive = useUiStore((s) => s.searchActive);

  const handleSidebarDrag = useCallback(
    (delta: number) => {
      setSidebarWidth(Math.max(140, Math.min(400, sidebarWidth + delta)));
    },
    [sidebarWidth, setSidebarWidth],
  );

  const handleMessageListDrag = useCallback(
    (delta: number) => {
      setMessageListWidth(
        Math.max(280, Math.min(700, messageListWidth + delta)),
      );
    },
    [messageListWidth, setMessageListWidth],
  );

  return (
    <div className="flex h-screen w-full overflow-hidden">
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
        className="flex shrink-0 flex-col overflow-hidden border-x border-border"
        style={{ width: messageListWidth }}
      >
        <SearchBar />
        {searchActive ? (
          <SearchResults />
        ) : (
          <div className="flex-1 overflow-y-auto">{messageList}</div>
        )}
      </main>

      {/* Resize handle: message list | reading pane */}
      <ResizeHandle onDrag={handleMessageListDrag} />

      {/* Right panel — action bar + reading pane (fills remaining space) */}
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
    </div>
  );
}
