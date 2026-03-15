"use client";

import { useState } from "react";
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
  Loader2,
} from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
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
  buildReplyBodyHtml,
  buildForwardBodyHtml,
  buildReferences,
} from "@/lib/email-utils";
import type { EmailAddress } from "@/types/message";
import { useIdentities } from "@/hooks/useIdentities";
import type { Identity } from "@/types/identity";
import { createFadeSlideVariants, createScaleFadeVariants } from "@/lib/motion/variants";

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
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const shouldAnimate = effectiveAnimationMode !== "off";
  const barMotionProps = createFadeSlideVariants(effectiveAnimationMode, "y");
  const feedbackMotionProps = createScaleFadeVariants(effectiveAnimationMode);
  const ActionContainer = shouldAnimate ? motion.div : "div";
  const [actionFeedback, setActionFeedback] = useState<"delete" | "archive" | "move" | null>(null);

  const isSeen = data?.flags.includes("\\Seen") ?? false;
  const isFlagged = data?.flags.includes("\\Flagged") ?? false;

  // Reply All is redundant for 1-to-1 conversations (no other recipients besides sender + me)
  const isDirectConversation = (() => {
    if (!data) return false;
    const myEmail = useAuthStore.getState().activeAccount()?.email ?? "";
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
    const hasHtml = !!(data.html && data.html.trim());
    useComposeStore.getState().openReply({
      to: data.from_address,
      cc: "",
      subject: buildReplySubject(data.subject),
      body: hasHtml
        ? buildReplyBodyHtml(data.html!, data.from_address, data.date)
        : buildReplyBody(data.text, data.from_address, data.date),
      inReplyTo: messageId,
      references: buildReferences(refs, messageId),
      fromIdentityId: matchedId,
      isHtml: hasHtml,
    });
  };

  const handleReplyAll = () => {
    if (!data) return;
    const myEmail = useAuthStore.getState().activeAccount()?.email ?? "";
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
    const hasHtml = !!(data.html && data.html.trim());
    useComposeStore.getState().openReply({
      to: replyTo,
      cc: ccList,
      subject: buildReplySubject(data.subject),
      body: hasHtml
        ? buildReplyBodyHtml(data.html!, data.from_address, data.date)
        : buildReplyBody(data.text, data.from_address, data.date),
      inReplyTo: messageId,
      references: buildReferences(refs, messageId),
      fromIdentityId: matchedId,
      isHtml: hasHtml,
    });
  };

  const handleForward = () => {
    if (!data) return;
    const toList = formatAddressList(data.to_addresses);
    const hasHtml = !!(data.html && data.html.trim());
    useComposeStore.getState().openForward({
      subject: buildForwardSubject(data.subject),
      body: hasHtml
        ? buildForwardBodyHtml(data.html!, data.from_address, data.date, data.subject, toList)
        : buildForwardBody(data.text, data.from_address, data.date, data.subject, toList),
      isHtml: hasHtml,
    });
  };

  const handleDelete = () => {
    if (!data) return;
    setActionFeedback("delete");
    if (activeFolder === "Trash") {
      deleteMessage.mutate(
        { folder: activeFolder, uid: data.uid },
        { onSettled: () => setActionFeedback(null) },
      );
    } else {
      moveMessage.mutate(
        { fromFolder: activeFolder, toFolder: "Trash", uid: data.uid },
        { onSettled: () => setActionFeedback(null) },
      );
    }
  };

  const handleArchive = () => {
    if (!data) return;
    setActionFeedback("archive");
    moveMessage.mutate(
      { fromFolder: activeFolder, toFolder: "Archive", uid: data.uid },
      { onSettled: () => setActionFeedback(null) },
    );
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
    <ActionContainer
      {...(shouldAnimate
        ? {
            "data-testid": "message-action-bar-transition",
            "data-motion-props": JSON.stringify(barMotionProps),
            initial: "initial",
            animate: "animate",
            exit: "exit",
            variants: barMotionProps,
          }
        : {})}
      className="flex shrink-0 flex-wrap items-center gap-0.5 border-b border-border px-2 py-1"
    >
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
        {shouldAnimate ? (
          <AnimatePresence mode="wait" initial={false}>
            <motion.span
              key={actionFeedback === "delete" ? "delete-busy" : "delete-idle"}
              data-testid="message-action-delete-feedback-transition"
              data-motion-props={JSON.stringify(feedbackMotionProps)}
              initial="initial"
              animate="animate"
              exit="exit"
              variants={feedbackMotionProps}
              className="inline-flex"
            >
              {actionFeedback === "delete" ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <Trash2 className="size-4" />
              )}
            </motion.span>
          </AnimatePresence>
        ) : (
          <Trash2 className="size-4" />
        )}
        <span className="hidden xl:inline">{activeFolder === "Trash" ? "Delete" : "Delete"}</span>
      </Button>

      {/* Archive */}
      <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled={disabled || activeFolder === "Archive"} onClick={handleArchive}>
        {shouldAnimate ? (
          <AnimatePresence mode="wait" initial={false}>
            <motion.span
              key={actionFeedback === "archive" ? "archive-busy" : "archive-idle"}
              data-testid="message-action-archive-feedback-transition"
              data-motion-props={JSON.stringify(feedbackMotionProps)}
              initial="initial"
              animate="animate"
              exit="exit"
              variants={feedbackMotionProps}
              className="inline-flex"
            >
              {actionFeedback === "archive" ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <Archive className="size-4" />
              )}
            </motion.span>
          </AnimatePresence>
        ) : (
          <Archive className="size-4" />
        )}
        <span className="hidden xl:inline">Archive</span>
      </Button>

      {/* Move to */}
      {shouldAnimate ? (
        <AnimatePresence mode="wait" initial={false}>
          <motion.span
            key={actionFeedback === "move" ? "move-busy" : "move-idle"}
            data-testid="message-action-move-feedback-transition"
            data-motion-props={JSON.stringify(feedbackMotionProps)}
            initial="initial"
            animate="animate"
            exit="exit"
            variants={feedbackMotionProps}
            className="inline-flex"
          >
            {actionFeedback === "move" ? (
              <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled>
                <Loader2 className="size-4 animate-spin" />
                <span className="hidden xl:inline">Move to...</span>
              </Button>
            ) : disabled ? (
              <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled>
                <span className="hidden xl:inline">Move to...</span>
              </Button>
            ) : (
              <MoveToFolderMenu
                currentFolder={activeFolder}
                onMove={(toFolder) => {
                  setActionFeedback("move");
                  moveMessage.mutate(
                    { fromFolder: activeFolder, toFolder, uid: data.uid },
                    { onSettled: () => setActionFeedback(null) },
                  );
                }}
              />
            )}
          </motion.span>
        </AnimatePresence>
      ) : disabled ? (
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
        {shouldAnimate ? (
          <AnimatePresence mode="wait" initial={false}>
            <motion.span
              key={isFlagged ? "flagged" : "unflagged"}
              data-testid="message-action-star-feedback-transition"
              data-motion-props={JSON.stringify(feedbackMotionProps)}
              initial="initial"
              animate="animate"
              exit="exit"
              variants={feedbackMotionProps}
              className="inline-flex"
            >
              {isFlagged ? (
                <Star className="size-4 fill-primary text-primary" />
              ) : (
                <Star className="size-4" />
              )}
            </motion.span>
          </AnimatePresence>
        ) : isFlagged ? (
          <Star className="size-4 fill-primary text-primary" />
        ) : (
          <Star className="size-4" />
        )}
        <span className="hidden xl:inline">{isFlagged ? "Unstar" : "Star"}</span>
      </Button>

      {/* Mark read/unread */}
      <Button variant="ghost" size="sm" className="shrink-0 gap-1.5" disabled={disabled} onClick={handleToggleRead}>
        {shouldAnimate ? (
          <AnimatePresence mode="wait" initial={false}>
            <motion.span
              key={isSeen ? "seen" : "unseen"}
              data-testid="message-action-read-feedback-transition"
              data-motion-props={JSON.stringify(feedbackMotionProps)}
              initial="initial"
              animate="animate"
              exit="exit"
              variants={feedbackMotionProps}
              className="inline-flex"
            >
              {isSeen ? (
                <MailOpen className="size-4" />
              ) : (
                <Mail className="size-4" />
              )}
            </motion.span>
          </AnimatePresence>
        ) : isSeen ? (
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
    </ActionContainer>
  );
}
