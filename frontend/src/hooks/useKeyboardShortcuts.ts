"use client";

import { useEffect } from "react";
import { useUiStore } from "@/stores/useUiStore";
import {
  useUpdateFlags,
  useMoveMessage,
  useDeleteMessage,
  useMessages,
} from "@/hooks/useMessages";

function isInputFocused(): boolean {
  const el = document.activeElement;
  if (!el) return false;
  const tag = el.tagName.toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return true;
  if ((el as HTMLElement).isContentEditable) return true;
  return false;
}

export function useKeyboardShortcuts() {
  const activeFolder = useUiStore((s) => s.activeFolder);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const selectMessage = useUiStore((s) => s.selectMessage);
  const searchActive = useUiStore((s) => s.searchActive);
  const setSearchActive = useUiStore((s) => s.setSearchActive);
  const clearSearch = useUiStore((s) => s.clearSearch);

  const updateFlags = useUpdateFlags();
  const moveMessage = useMoveMessage();
  const deleteMessage = useDeleteMessage();
  const { data } = useMessages(activeFolder);

  // Flatten all pages into a single array.
  const messages = data?.pages.flatMap((page) => page.messages) ?? [];

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Cmd/Ctrl+K — search
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setSearchActive(true);
        setTimeout(() => {
          const searchInput = document.querySelector(
            "[data-search-input]",
          ) as HTMLElement;
          searchInput?.focus();
        }, 0);
        return;
      }

      // Don't handle other shortcuts when typing in inputs
      if (isInputFocused()) return;

      // Escape — close reading pane or clear search
      if (e.key === "Escape") {
        if (searchActive) {
          clearSearch();
        } else if (selectedMessageUid !== null) {
          selectMessage(null);
        }
        return;
      }

      // All remaining shortcuts require a selected message
      if (selectedMessageUid === null) {
        // J/ArrowDown with no selection — select first message
        if (
          (e.key === "j" || e.key === "ArrowDown") &&
          messages.length > 0
        ) {
          e.preventDefault();
          selectMessage(messages[0].uid);
        }
        return;
      }

      const currentIndex = messages.findIndex(
        (m) => m.uid === selectedMessageUid,
      );

      switch (e.key) {
        case "Delete":
        case "Backspace":
          e.preventDefault();
          if (activeFolder === "Trash") {
            deleteMessage.mutate(
              { folder: activeFolder, uid: selectedMessageUid },
              { onSuccess: () => selectMessage(null) },
            );
          } else {
            moveMessage.mutate(
              {
                fromFolder: activeFolder,
                toFolder: "Trash",
                uid: selectedMessageUid,
              },
              { onSuccess: () => selectMessage(null) },
            );
          }
          break;

        case "s":
          if (currentIndex >= 0) {
            const flagged =
              messages[currentIndex].flags.includes("\\Flagged");
            updateFlags.mutate({
              folder: activeFolder,
              uid: selectedMessageUid,
              flags: ["\\Flagged"],
              add: !flagged,
            });
          }
          break;

        case "u":
          if (currentIndex >= 0) {
            const seen = messages[currentIndex].flags.includes("\\Seen");
            updateFlags.mutate({
              folder: activeFolder,
              uid: selectedMessageUid,
              flags: ["\\Seen"],
              add: !seen,
            });
          }
          break;

        case "j":
        case "ArrowDown":
          e.preventDefault();
          if (currentIndex >= 0 && currentIndex < messages.length - 1) {
            selectMessage(messages[currentIndex + 1].uid);
          }
          break;

        case "k":
        case "ArrowUp":
          e.preventDefault();
          if (currentIndex > 0) {
            selectMessage(messages[currentIndex - 1].uid);
          }
          break;
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [
    activeFolder,
    selectedMessageUid,
    selectMessage,
    searchActive,
    setSearchActive,
    clearSearch,
    updateFlags,
    moveMessage,
    deleteMessage,
    messages,
  ]);
}
