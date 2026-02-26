"use client";

import { ThreePanelLayout } from "@/components/shared/ThreePanelLayout";
import { NavRail } from "@/components/shared/NavRail";
import { FolderTree } from "@/components/mail/FolderTree";
import { MessageList } from "@/components/mail/MessageList";
import { ReadingPane } from "@/components/mail/ReadingPane";
import { ComposeDialog } from "@/components/mail/ComposeDialog";

export default function MailPage() {
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
