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

export default function MailPage() {
  const viewMode = useUiStore((s) => s.viewMode);
  useKeyboardShortcuts();
  useWebSocket();

  if (viewMode === "contacts") {
    return (
      <div className="flex h-screen w-full overflow-hidden">
        <NavRail />
        <ContactsPanel />
      </div>
    );
  }

  if (viewMode === "settings") {
    return (
      <div className="flex h-screen w-full overflow-hidden">
        <NavRail />
        <SettingsPanel />
      </div>
    );
  }

  return (
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
