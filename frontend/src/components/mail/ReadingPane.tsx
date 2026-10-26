"use client";

import { apiUrl } from "@/lib/api-url";

import { useState, useEffect, useRef, useMemo } from "react";
import { AnimatePresence } from "framer-motion";
import {
  Paperclip,
  ChevronDown,
  ChevronUp,
  Code,
  Type,
  FileCode,
  ShieldAlert,
  Sun,
  Moon,
  Monitor,
} from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useUiStore } from "@/stores/useUiStore";
import { useAuthStore } from "@/stores/useAuthStore";
import {
  normalizeSenderAddress,
  remoteResourceAccountScope,
  useRemoteResourceStore,
} from "@/stores/useRemoteResourceStore";
import { useMessage, useUpdateFlags } from "@/hooks/useMessages";
import { createFadeSlideVariants } from "@/lib/motion/variants";
import { AnimatedDiv } from "@/lib/motion/AnimatedDiv";
import { EmailRenderer, hasRemoteResources } from "./EmailRenderer";
import { ThreadView } from "./ThreadView";
import { Button } from "@/components/ui/button";
import {
  AddressChip,
  AddressList,
  AttachmentPreviewer,
  HeaderSkeleton,
  BodySkeleton,
  formatFileSize,
  humanizeDate,
} from "./ReadingPane/index";

type BodyMode = "html" | "plain";

export function ReadingPane() {
  const activeFolder = useUiStore((s) => s.activeFolder);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const activeAccountId = useAuthStore((s) => s.activeAccountId);
  const activeAccount = useAuthStore((s) => s.accounts.find((a) => a.id === s.activeAccountId));
  const accountScope = remoteResourceAccountScope(activeAccount);
  const trustedSenders = useRemoteResourceStore((s) => accountScope ? s.sendersByAccount[accountScope] : undefined);
  const loadTrustedSenders = useRemoteResourceStore((s) => s.loadAccount);
  const trustSender = useRemoteResourceStore((s) => s.trustSender);
  useEffect(() => {
    if (accountScope) loadTrustedSenders(accountScope);
  }, [accountScope, loadTrustedSenders]);

  const selectedMessageKey = JSON.stringify([activeAccountId, activeFolder, selectedMessageUid]);
  const [messageView, setMessageView] = useState<{
    key: string;
    bodyMode: BodyMode | null;
    emailTheme: "auto" | "light" | "dark";
    senderDetailsOpen: boolean;
  }>({ key: selectedMessageKey, bodyMode: null, emailTheme: "auto", senderDetailsOpen: false });
  if (messageView.key !== selectedMessageKey) {
    setMessageView({ key: selectedMessageKey, bodyMode: null, emailTheme: "auto", senderDetailsOpen: false });
  }
  const [showHeaders, setShowHeaders] = useState(false);
  const emailTheme = messageView.emailTheme;
  const [previewIndex, setPreviewIndex] = useState<number | null>(null);

  const queryClient = useQueryClient();
  const selectMessage = useUiStore((s) => s.selectMessage);

  const { data, isLoading, isError, error, refetch } = useMessage(
    activeFolder,
    selectedMessageUid ?? 0
  );

  const updateFlags = useUpdateFlags();

  const paneVariants = useMemo(() => createFadeSlideVariants(effectiveAnimationMode, "x"), [effectiveAnimationMode]);

  // When a message is not found on the server (stale cache), deselect it and
  // refresh the message list and search results so the ghost entry disappears.
  const handledStaleRef = useRef<number | null>(null);
  useEffect(() => {
    if (!isError || !error?.message?.includes("not found")) return;
    if (handledStaleRef.current === selectedMessageUid) return;
    handledStaleRef.current = selectedMessageUid;
    selectMessage(null);
    queryClient.invalidateQueries({ queryKey: ["messages", activeFolder] });
    queryClient.invalidateQueries({ queryKey: ["folders"] });
    queryClient.invalidateQueries({ queryKey: ["search"] });
  }, [isError, error, selectedMessageUid, activeFolder, selectMessage, queryClient]);

  const bodyMode = messageView.bodyMode ?? (
    data && !data.html && data.text ? "plain" : "html"
  );

  // Auto-mark unread messages as read when opened.
  useEffect(() => {
    if (data && !data.flags.includes("\\Seen")) {
      updateFlags.mutate({
        folder: activeFolder,
        uid: data.uid,
        flags: ["\\Seen"],
        add: true,
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data?.uid, data?.folder]);

  // No message selected
  if (selectedMessageUid === null) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        Select a message to read
      </div>
    );
  }

  // Loading
  if (isLoading) {
    return (
      <div className="flex h-full flex-col overflow-y-auto">
        <HeaderSkeleton />
        <BodySkeleton />
      </div>
    );
  }

  // Error
  if (isError || !data) {
    const errMsg = error?.message ?? "Unknown error";
    console.error(`[ReadingPane] Failed to load message uid=${selectedMessageUid} folder=${activeFolder}:`, errMsg);
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-4 py-8 text-center">
        <p className="text-sm text-muted-foreground">Failed to load message</p>
        <p className="max-w-md text-xs text-muted-foreground/60">{errMsg}</p>
        <Button variant="outline" size="sm" onClick={() => refetch()}>
          Retry
        </Button>
      </div>
    );
  }

  const attachmentBaseUrl = apiUrl(`/messages/${encodeURIComponent(data.folder)}/${data.uid}/attachments`);
  const senderAddress = normalizeSenderAddress(data.from_address);
  const remoteAllowed = senderAddress !== null && trustedSenders?.includes(senderAddress) === true;
  const showRemoteBanner = !remoteAllowed && hasRemoteResources(data.html);

  const paneContent = (
    <div
      className="flex h-full w-full flex-col overflow-hidden"
    >
      {/* Header area */}
      <div className="shrink-0 space-y-1 overflow-x-hidden border-b border-border p-4">
        <h2 className="text-lg font-bold leading-tight">{data.subject}</h2>

        <div className="text-sm text-foreground">
          From{" "}
          <button
            type="button"
            aria-expanded={messageView.senderDetailsOpen}
            aria-controls="sender-details"
            onClick={() => setMessageView({ ...messageView, senderDetailsOpen: !messageView.senderDetailsOpen })}
            className="inline-flex items-center gap-1 rounded px-0.5 text-sm text-foreground underline decoration-muted-foreground/30 underline-offset-2 hover:bg-accent hover:decoration-foreground"
          >
            {data.from_name || data.from_address}
            {messageView.senderDetailsOpen ? <ChevronUp className="size-3" /> : <ChevronDown className="size-3" />}
          </button>
          {" "}on {humanizeDate(data.date)}
        </div>

        {messageView.senderDetailsOpen && (
          <div id="sender-details" role="region" aria-label="Sender details" className="space-y-1">
            <div className="text-sm text-foreground">
              <span className="font-medium text-muted-foreground">From: </span>
              <AddressChip address={data.from_address} name={data.from_name || null} />
              {data.from_name && <span className="ml-1 text-muted-foreground">&lt;{data.from_address}&gt;</span>}
            </div>
            <div className="text-sm text-foreground">
              <span className="font-medium text-muted-foreground">To: </span>
              <AddressList addresses={data.to_addresses} />
            </div>
            {data.cc_addresses.length > 0 && (
              <div className="text-sm text-foreground">
                <span className="font-medium text-muted-foreground">Cc: </span>
                <AddressList addresses={data.cc_addresses} />
              </div>
            )}
          </div>
        )}

        {/* View toggle buttons */}
        <div className="flex gap-1 pt-1">
          {/* Plain text / HTML toggle */}
          <button
            type="button"
            onClick={() => {
              setMessageView({
                ...messageView,
                bodyMode: bodyMode === "html" ? "plain" : "html",
              });
              setShowHeaders(false);
            }}
            className="inline-flex items-center gap-1 rounded px-2 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            {bodyMode === "html" ? (
              <Type className="size-3" />
            ) : (
              <Code className="size-3" />
            )}
            {bodyMode === "html" ? "Plain text" : "HTML"}
          </button>

          {/* Headers toggle (shows as selected when active) */}
          <button
            type="button"
            onClick={() => setShowHeaders(!showHeaders)}
            className={`inline-flex items-center gap-1 rounded px-2 py-0.5 text-xs transition-colors ${
              showHeaders
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-muted hover:text-foreground"
            }`}
          >
            <FileCode className="size-3" />
            Headers
          </button>

          {/* Email Theme toggle */}
          <button
            type="button"
            onClick={() => {
              setMessageView({
                ...messageView,
                emailTheme:
                  emailTheme === "auto" ? "light" : emailTheme === "light" ? "dark" : "auto",
              });
            }}
            className={`inline-flex items-center gap-1 rounded px-2 py-0.5 text-xs transition-colors ${
              emailTheme !== "auto"
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-muted hover:text-foreground"
            }`}
          >
            {emailTheme === "auto" && <Monitor className="size-3" />}
            {emailTheme === "light" && <Sun className="size-3" />}
            {emailTheme === "dark" && <Moon className="size-3" />}
            {emailTheme === "auto" ? "Auto theme" : emailTheme === "light" ? "Light mode" : "Dark mode"}
          </button>
        </div>
      </div>

      {/* Attachment bar */}
      {data.attachments.length > 0 && (
        <div className="flex shrink-0 gap-2 overflow-x-auto border-b border-border px-4 py-2">
          {data.attachments.map((att, i) => (
            <button
              key={att.id}
              type="button"
              onClick={() => setPreviewIndex(i)}
              className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-border bg-muted/50 px-2.5 py-1 text-xs text-foreground transition-colors hover:bg-muted"
            >
              <Paperclip className="size-3.5 shrink-0 text-muted-foreground" />
              <span className="max-w-[200px] truncate">
                {att.filename ?? "Attachment"}
              </span>
              <span className="text-muted-foreground">
                ({formatFileSize(att.size)})
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Attachment previewer */}
      {previewIndex !== null && (
        <AttachmentPreviewer
          attachments={data.attachments}
          baseUrl={attachmentBaseUrl}
          accountId={activeAccountId}
          initialIndex={previewIndex}
          onClose={() => setPreviewIndex(null)}
        />
      )}

      {/* Thread view — only shown when there are multiple messages in the thread */}
      {data.thread && data.thread.length > 1 && (() => {
        const visibleThread = data.thread.filter(
          (m) => m.folder === activeFolder || m.uid === data.uid,
        );
        return visibleThread.length > 1 ? (
          <ThreadView thread={visibleThread} currentUid={data.uid} />
        ) : null;
      })()}

      {/* Remote resources banner */}
      {showRemoteBanner && (
        <div className="flex shrink-0 items-center gap-2 border-b border-border bg-muted/50 px-4 py-2">
          <ShieldAlert className="size-4 shrink-0 text-muted-foreground" />
          <span className="flex-1 text-xs text-muted-foreground">
            To protect your privacy, remote resources have been blocked.
          </span>
          <Button
            variant="outline"
            size="sm"
            className="h-6 text-xs"
            disabled={!accountScope || !senderAddress}
            onClick={() => {
              if (accountScope) trustSender(accountScope, data.from_address);
            }}
          >
            Always allow from this sender
          </Button>
        </div>
      )}

      {/* Body area — fills remaining space */}
      <div className="min-h-0 flex-1" key={data.uid}>
        {showHeaders ? (
          <pre className="h-full overflow-auto whitespace-pre-wrap break-words p-4 text-xs leading-relaxed text-foreground">
            {data.raw_headers || "No headers available"}
          </pre>
        ) : bodyMode === "plain" ? (
          <pre className="h-full overflow-auto whitespace-pre-wrap break-words p-4 text-sm leading-relaxed text-foreground">
            {data.text || "No plain text available"}
          </pre>
        ) : (
          <EmailRenderer
            html={data.html}
            text={data.text}
            blockRemoteResources={!remoteAllowed}
            theme={emailTheme}
            emailTheme={data.email_theme}
          />
        )}
      </div>
    </div>
  );

  return (
    <AnimatePresence mode="wait" initial={false}>
      <AnimatedDiv
        key={`reading-pane-${selectedMessageKey}`}
        data-testid="reading-pane-message-transition"
        variants={paneVariants}
        initial={paneVariants.initial}
        animate={paneVariants.animate}
        exit={paneVariants.exit}
        className="h-full min-w-0 w-full"
      >
        {paneContent}
      </AnimatedDiv>
    </AnimatePresence>
  );
}
