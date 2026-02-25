"use client";

import { ThreePanelLayout } from "@/components/shared/ThreePanelLayout";
import { FolderTree } from "@/components/mail/FolderTree";
import { MessageList } from "@/components/mail/MessageList";
import { ReadingPane } from "@/components/mail/ReadingPane";

export default function MailPage() {
  return (
    <ThreePanelLayout
      sidebar={<FolderTree />}
      messageList={<MessageList />}
      readingPane={<ReadingPane />}
    />
  );
}
