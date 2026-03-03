"use client";

import {
  Reply,
  ReplyAll,
  Forward,
  Trash2,
  Archive,
  Star,
  Mail,
  MailOpen,
  AlertCircle,
} from "lucide-react";
import { useUiStore } from "@/stores/useUiStore";
import {
  useMessage,
  useUpdateFlags,
  useMoveMessage,
  useDeleteMessage,
} from "@/hooks/useMessages";
import { MoveToFolderMenu } from "./MoveToFolderMenu";
import { TagPicker } from "./TagPicker";
import { Button } from "@/components/ui/button";
import { useComposeStore } from "@/stores/useComposeStore";
import { useAuthStore } from "@/stores/useAuthStore";
import {
  extractHeader,
  buildReplySubject,
  buildForwardSubject,
  buildReplyBody,
  buildForwardBody,
  buildReferences,
} from "@/lib/email-utils";
import type { EmailAddress } from "@/types/message";
import { useIdentities } from "@/hooks/useIdentities";
import type { Identity } from "@/types/identity";

/** Find the identity whose email matches one of the To/CC addresses. */
function findMatchingIdentity(
  identities: Identity[] | undefined,
  toAddresses: EmailAddress[],
  ccAddresses: EmailAddress[],
): number | null {
  if (!identities || identities.length === 0) return null;
  const allRecipientEmails = [...toAddresses, ...ccAddresses].map((a) =>
    a.address.toLowerCase(),
  );
  const match = identities.find((i) =>
    allRecipientEmails.includes(i.email.toLowerCase()),
  );
  return match?.id ?? null;
}

function formatAddressList(addresses: EmailAddress[]): string {
  return addresses
    .map((a) => (a.name ? `${a.name} <${a.address}>` : a.address))
    .join(", ");
}

export function MessageActionBar() {
  const activeFolder = useUiStore((s) => s.activeFolder);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const updateFlags = useUpdateFlags();
  const moveMessage = useMoveMessage();
  const deleteMessage = useDeleteMessage();

  const { data } = useMessage(activeFolder, selectedMessageUid ?? 0);
  const { data: identities } = useIdentities();

  const disabled = !data;

  const isSeen = data?.flags.includes("\\Seen") ?? false;
  const isFlagged = data?.flags.includes("\\Flagged") ?? false;

  // Reply All is redundant for 1-to-1 conversations (no other recipients besides sender + me)
  const isDirectConversation = (() => {
    if (!data) return false;
    const myEmail = useAuthStore.getState().email ?? "";
    const otherRecipients = [...data.to_addresses, ...data.cc_addresses].filter(
      (a) =>
        a.address.toLowerCase() !== myEmail.toLowerCase() &&
        a.address.toLowerCase() !== data.from_address.toLowerCase(),
    );
    return otherRecipients.length === 0;
  })();

  const handleReply = () => {
    if (!data) return;
    const messageId = extractHeader(data.raw_headers, "Message-ID");
    const refs = extractHeader(data.raw_headers, "References");
    const matchedId = findMatchingIdentity(identities, data.to_addresses, data.cc_addresses);
    useComposeStore.getState().openReply({
      to: data.from_address,
      cc: "",
      subject: buildReplySubject(data.subject),
      body: buildReplyBody(data.text, data.from_address, data.date),
      inReplyTo: messageId,
      references: buildReferences(refs, messageId),
      fromIdentityId: matchedId,
    });
  };

  const handleReplyAll = () => {
    if (!data) return;
    const myEmail = useAuthStore.getState().email ?? "";
    const messageId = extractHeader(data.raw_headers, "Message-ID");
    const refs = extractHeader(data.raw_headers, "References");
    const replyTo = data.from_address;
    const allRecipients = [
      ...data.to_addresses,
      ...data.cc_addresses,
    ].filter(
      (a) =>
        a.address.toLowerCase() !== myEmail.toLowerCase() &&
        a.address.toLowerCase() !== data.from_address.toLowerCase(),
    );
    const ccList = allRecipients.map((a) => a.address).join(", ");
    const matchedId = findMatchingIdentity(identities, data.to_addresses, data.cc_addresses);
    useComposeStore.getState().openReply({
      to: replyTo,
      cc: ccList,
      subject: buildReplySubject(data.subject),
      body: buildReplyBody(data.text, data.from_address, data.date),
      inReplyTo: messageId,
      references: buildReferences(refs, messageId),
      fromIdentityId: matchedId,
    });
  };

  const handleForward = () => {
    if (!data) return;
    const toList = formatAddressList(data.to_addresses);
    useComposeStore.getState().openForward({
      subject: buildForwardSubject(data.subject),
      body: buildForwardBody(
        data.text,
        data.from_address,
        data.date,
        data.subject,
        toList,
      ),
    });
  };

  const handleDelete = () => {
    if (!data) return;
    if (activeFolder === "Trash") {
      deleteMessage.mutate({ folder: activeFolder, uid: data.uid });
    } else {
      moveMessage.mutate({ fromFolder: activeFolder, toFolder: "Trash", uid: data.uid });
    }
  };

  const handleArchive = () => {
    if (!data) return;
    moveMessage.mutate({ fromFolder: activeFolder, toFolder: "Archive", uid: data.uid });
  };

  const handleJunk = () => {
    if (!data) return;
    moveMessage.mutate({ fromFolder: activeFolder, toFolder: "Junk", uid: data.uid });
  };

  const handleToggleStar = () => {
    if (!data) return;
    updateFlags.mutate({
      folder: activeFolder,
      uid: data.uid,
      flags: ["\\Flagged"],
      add: !isFlagged,
    });
  };

  const handleToggleRead = () => {
    if (!data) return;
    updateFlags.mutate({
      folder: activeFolder,
      uid: data.uid,
      flags: ["\\Seen"],
      add: !isSeen,
    });
  };

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-0.5 border-b border-border px-2 py-1">
      {/* Reply */}
      <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled={disabled} onClick={handleReply}>
        <Reply className="size-4" />
        <span className="hidden xl:inline">Reply</span>
      </Button>

      {/* Reply All — disabled for 1-to-1 conversations */}
      <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled={disabled || isDirectConversation} onClick={handleReplyAll}>
        <ReplyAll className="size-4" />
        <span className="hidden xl:inline">Reply all</span>
      </Button>

      {/* Forward */}
      <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled={disabled} onClick={handleForward}>
        <Forward className="size-4" />
        <span className="hidden xl:inline">Forward</span>
      </Button>

      <div className="mx-0.5 h-5 w-px shrink-0 bg-border" />

      {/* Delete */}
      <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled={disabled} onClick={handleDelete}>
        <Trash2 className="size-4" />
        <span className="hidden xl:inline">{activeFolder === "Trash" ? "Delete" : "Delete"}</span>
      </Button>

      {/* Archive */}
      <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled={disabled || activeFolder === "Archive"} onClick={handleArchive}>
        <Archive className="size-4" />
        <span className="hidden xl:inline">Archive</span>
      </Button>

      {/* Move to */}
      {disabled ? (
        <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled>
          <span className="hidden xl:inline">Move to...</span>
        </Button>
      ) : (
        <MoveToFolderMenu
          currentFolder={activeFolder}
          onMove={(toFolder) => {
            moveMessage.mutate({ fromFolder: activeFolder, toFolder, uid: data.uid });
          }}
        />
      )}

      {/* Junk */}
      <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled={disabled} onClick={handleJunk}>
        <AlertCircle className="size-4" />
        <span className="hidden xl:inline">Junk</span>
      </Button>

      {/* Star/Unstar */}
      <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled={disabled} onClick={handleToggleStar}>
        {isFlagged ? (
          <Star className="size-4 fill-primary text-primary" />
        ) : (
          <Star className="size-4" />
        )}
        <span className="hidden xl:inline">{isFlagged ? "Unstar" : "Star"}</span>
      </Button>

      {/* Mark read/unread */}
      <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled={disabled} onClick={handleToggleRead}>
        {isSeen ? (
          <MailOpen className="size-4" />
        ) : (
          <Mail className="size-4" />
        )}
        <span className="hidden xl:inline">{isSeen ? "Unread" : "Read"}</span>
      </Button>

      {/* Tags */}
      {data && (
        <TagPicker folder={activeFolder} uid={data.uid} />
      )}
    </div>
  );
}
