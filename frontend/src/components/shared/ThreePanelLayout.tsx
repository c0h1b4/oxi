"use client";

import { useUiStore } from "@/stores/useUiStore";
import { cn } from "@/lib/utils";

interface ThreePanelLayoutProps {
  sidebar: React.ReactNode;
  messageList: React.ReactNode;
  readingPane: React.ReactNode;
}

export function ThreePanelLayout({
  sidebar,
  messageList,
  readingPane,
}: ThreePanelLayoutProps) {
  const sidebarWidth = useUiStore((s) => s.sidebarWidth);
  const readingPaneVisible = useUiStore((s) => s.readingPaneVisible);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);

  const showReadingPane = readingPaneVisible && selectedMessageUid !== null;

  return (
    <div className="flex h-screen w-full overflow-hidden">
      {/* Left sidebar */}
      <aside
        className="flex-none overflow-y-auto border-r border-border bg-sidebar"
        style={{ width: sidebarWidth }}
      >
        {sidebar}
      </aside>

      {/* Center panel — message list */}
      <main
        className={cn(
          "min-w-[300px] flex-1 overflow-y-auto",
          showReadingPane && "max-w-[50%]",
        )}
      >
        {messageList}
      </main>

      {/* Right panel — reading pane */}
      {showReadingPane && (
        <section className="flex-1 overflow-y-auto border-l border-border">
          {readingPane}
        </section>
      )}
    </div>
  );
}
