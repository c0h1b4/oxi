"use client";

import { ThreePanelLayout } from "@/components/shared/ThreePanelLayout";
import { FolderTree } from "@/components/mail/FolderTree";

export default function MailPage() {
  return (
    <ThreePanelLayout
      sidebar={<FolderTree />}
      messageList={
        <div className="flex h-full items-center justify-center text-muted-foreground">
          Message list coming soon
        </div>
      }
      readingPane={
        <div className="flex h-full items-center justify-center text-muted-foreground">
          Select a message to read
        </div>
      }
    />
  );
}
