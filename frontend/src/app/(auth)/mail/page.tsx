"use client";

import { useMemo } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { ThreePanelLayout } from "@/components/shared/ThreePanelLayout";
import { NavRail } from "@/components/shared/NavRail";
import { FolderTree } from "@/components/mail/FolderTree";
import { MessageList } from "@/components/mail/MessageList";
import { ReadingPane } from "@/components/mail/ReadingPane";
import { CalendarPanel } from "@/components/calendar/CalendarPanel";
import { ContactsPanel } from "@/components/contacts/ContactsPanel";
import { SettingsPanel } from "@/components/settings/SettingsPanel";
import { useUiStore } from "@/stores/useUiStore";
import type { UiState } from "@/stores/useUiStore";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useWebSocket } from "@/hooks/useWebSocket";
import { useNotifications } from "@/hooks/useNotifications";
import { WsContext } from "@/lib/ws-context";
import { NotificationBanner } from "@/components/shared/NotificationBanner";
import { KeyboardShortcuts } from "@/components/shared/KeyboardShortcuts";
import { CommandPalette } from "@/components/shared/CommandPalette";
import { PreferencesLoader } from "@/components/PreferencesLoader";

export default function MailPage() {
  const viewMode = useUiStore((s: UiState) => s.viewMode);
  const effectiveAnimationMode = useUiStore((s: UiState) => s.effectiveAnimationMode);
  useKeyboardShortcuts();
  const { showBanner, requestPermission, dismissBanner, handleEvent } = useNotifications();
  const { status: wsStatus, failCount: wsFailCount } = useWebSocket(handleEvent);

  const wsContextValue = useMemo(
    () => ({ status: wsStatus, failCount: wsFailCount }),
    [wsStatus, wsFailCount],
  );

  const shouldAnimateViews = effectiveAnimationMode !== "off";

  let viewContent: React.ReactNode;
  if (viewMode === "contacts") {
    viewContent = (
      <div className="flex h-full min-h-0 w-full overflow-hidden">
        <NavRail />
        <ContactsPanel />
      </div>
    );
  } else if (viewMode === "calendar") {
    viewContent = (
      <div className="flex h-full min-h-0 w-full overflow-hidden">
        <NavRail />
        <CalendarPanel />
      </div>
    );
  } else if (viewMode === "settings") {
    viewContent = (
      <div className="flex h-full min-h-0 w-full overflow-hidden">
        <NavRail />
        <SettingsPanel />
      </div>
    );
  } else {
    viewContent = (
      <ThreePanelLayout
        navRail={<NavRail />}
        sidebar={<FolderTree />}
        messageList={<MessageList />}
        readingPane={<ReadingPane />}
      />
    );
  }

  const content = shouldAnimateViews ? (
    <AnimatePresence mode="sync" initial={false}>
      <motion.div
        key={viewMode}
        data-testid={`mail-view-transition-${viewMode}`}
        className="absolute inset-0"
        initial={{ opacity: 0, x: 6 }}
        animate={{ opacity: 1, x: 0, transition: { duration: 0.18, ease: [0.2, 0, 0, 1] as const } }}
        exit={{ opacity: 0, x: -4, transition: { duration: 0.12, ease: [0.2, 0, 0, 1] as const } }}
      >
        {viewContent}
      </motion.div>
    </AnimatePresence>
  ) : (
    <div data-testid={`mail-view-static-${viewMode}`} className="h-full min-h-0">
      {viewContent}
    </div>
  );

  return (
    <WsContext.Provider value={wsContextValue}>
      <PreferencesLoader />
      {showBanner && <NotificationBanner onEnable={requestPermission} onDismiss={dismissBanner} />}
      <div className="relative h-dvh w-full overflow-hidden">{content}</div>
      <KeyboardShortcuts />
      <CommandPalette />
    </WsContext.Provider>
  );
}
