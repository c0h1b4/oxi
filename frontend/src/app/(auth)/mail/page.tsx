"use client";

import { useMemo } from "react";
import { ThreePanelLayout } from "@/components/shared/ThreePanelLayout";
import { NavRail } from "@/components/shared/NavRail";
import { FolderTree } from "@/components/mail/FolderTree";
import { MessageList } from "@/components/mail/MessageList";
import { ReadingPane } from "@/components/mail/ReadingPane";
import { ComposeDialog } from "@/components/mail/ComposeDialog";
import { ContactsPanel } from "@/components/contacts/ContactsPanel";
import { SettingsPanel } from "@/components/settings/SettingsPanel";
import { useUiStore } from "@/stores/useUiStore";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useWebSocket } from "@/hooks/useWebSocket";
import { useNotifications } from "@/hooks/useNotifications";
import { WsContext } from "@/lib/ws-context";
import { NotificationBanner } from "@/components/shared/NotificationBanner";

export default function MailPage() {
  const viewMode = useUiStore((s) => s.viewMode);
  useKeyboardShortcuts();
  const { showBanner, requestPermission, dismissBanner, handleEvent } = useNotifications();
  const { status: wsStatus, failCount: wsFailCount } = useWebSocket(handleEvent);

  const wsContextValue = useMemo(
    () => ({ status: wsStatus, failCount: wsFailCount }),
    [wsStatus, wsFailCount],
  );

  let content;
  if (viewMode === "contacts") {
    content = (
      <div className="flex h-screen w-full overflow-hidden">
        <NavRail />
        <ContactsPanel />
      </div>
    );
  } else if (viewMode === "settings") {
    content = (
      <div className="flex h-screen w-full overflow-hidden">
        <NavRail />
        <SettingsPanel />
      </div>
    );
  } else {
    content = (
      <>
        <ThreePanelLayout
          navRail={<NavRail />}
          sidebar={<FolderTree />}
          messageList={<MessageList />}
          readingPane={<ReadingPane />}
        />
        <ComposeDialog />
      </>
    );
  }

  return (
    <WsContext.Provider value={wsContextValue}>
      {showBanner && <NotificationBanner onEnable={requestPermission} onDismiss={dismissBanner} />}
      {content}
    </WsContext.Provider>
  );
}
