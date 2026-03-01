"use client";

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

  if (viewMode === "contacts") {
    return (
      <WsContext.Provider value={{ status: wsStatus, failCount: wsFailCount }}>
        {showBanner && <NotificationBanner onEnable={requestPermission} onDismiss={dismissBanner} />}
        <div className="flex h-screen w-full overflow-hidden">
          <NavRail wsStatus={wsStatus} wsFailCount={wsFailCount} />
          <ContactsPanel />
        </div>
      </WsContext.Provider>
    );
  }

  if (viewMode === "settings") {
    return (
      <WsContext.Provider value={{ status: wsStatus, failCount: wsFailCount }}>
        {showBanner && <NotificationBanner onEnable={requestPermission} onDismiss={dismissBanner} />}
        <div className="flex h-screen w-full overflow-hidden">
          <NavRail wsStatus={wsStatus} wsFailCount={wsFailCount} />
          <SettingsPanel />
        </div>
      </WsContext.Provider>
    );
  }

  return (
    <WsContext.Provider value={{ status: wsStatus, failCount: wsFailCount }}>
      {showBanner && <NotificationBanner onEnable={requestPermission} onDismiss={dismissBanner} />}
      <ThreePanelLayout
        navRail={<NavRail wsStatus={wsStatus} wsFailCount={wsFailCount} />}
        sidebar={<FolderTree />}
        messageList={<MessageList />}
        readingPane={<ReadingPane />}
      />
      <ComposeDialog />
    </WsContext.Provider>
  );
}
