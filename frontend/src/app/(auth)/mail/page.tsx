"use client";

import { ThreePanelLayout } from "@/components/shared/ThreePanelLayout";
import { FolderTree } from "@/components/mail/FolderTree";
import { MessageList } from "@/components/mail/MessageList";

export default function MailPage() {
  return (
    <ThreePanelLayout
      sidebar={<FolderTree />}
      messageList={<MessageList />}
      readingPane={
        <div className="flex h-full items-center justify-center text-muted-foreground">
          Select a message to read
        </div>
      }
    />
  );
}
