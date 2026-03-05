"use client";

import { useEffect, useRef } from "react";
import {
  useQuery,
  useInfiniteQuery,
  useMutation,
  useQueryClient,
  keepPreviousData,
} from "@tanstack/react-query";
import type { InfiniteData } from "@tanstack/react-query";
import { apiGet, apiPatch, apiPost, apiDelete } from "@/lib/api";
import { useWsStatus } from "@/lib/ws-context";
import { useUiStore } from "@/stores/useUiStore";
import type { MessagesResponse, MessageDetail, MessageHeader } from "@/types/message";

const PER_PAGE = 50;

export function useMessages(folder: string) {
  const { status } = useWsStatus();
  return useInfiniteQuery({
    queryKey: ["messages", folder],
    queryFn: ({ pageParam = 0 }) =>
      apiGet<MessagesResponse>(
        `/folders/${encodeURIComponent(folder)}/messages?page=${pageParam}&per_page=${PER_PAGE}`,
      ),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => {
      const fetched = (lastPage.page + 1) * lastPage.per_page;
      return fetched < lastPage.total_count ? lastPage.page + 1 : undefined;
    },
    enabled: !!folder,
    refetchInterval: status === "connected" ? false : 60_000,
    placeholderData: keepPreviousData,
  });
}

export function useMessage(folder: string, uid: number) {
  return useQuery({
    queryKey: ["message", folder, uid],
    queryFn: () =>
      apiGet<MessageDetail>(
        `/messages/${encodeURIComponent(folder)}/${uid}`,
      ),
    enabled: !!folder && uid > 0,
    placeholderData: keepPreviousData,
  });
}

export function useUpdateFlags() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      folder,
      uid,
      flags,
      add,
    }: {
      folder: string;
      uid: number;
      flags: string[];
      add: boolean;
    }) =>
      apiPatch(`/messages/${encodeURIComponent(folder)}/${uid}/flags`, {
        flags,
        add,
      }),
    onSuccess: (_, { folder, uid }) => {
      queryClient.invalidateQueries({ queryKey: ["messages", folder] });
      queryClient.invalidateQueries({ queryKey: ["message", folder, uid] });
      queryClient.invalidateQueries({ queryKey: ["folders"] });
    },
  });
}

export function useMoveMessage() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      fromFolder,
      toFolder,
      uid,
    }: {
      fromFolder: string;
      toFolder: string;
      uid: number;
    }) =>
      apiPost("/messages/move", {
        from_folder: fromFolder,
        to_folder: toFolder,
        uid,
      }),
    onMutate: async ({ fromFolder, toFolder, uid }) => {
      // Auto-advance: if the moved message is selected, select the next (or previous) message.
      const { selectedMessageUid, selectMessage } = useUiStore.getState();
      if (selectedMessageUid === uid) {
        const prev = queryClient.getQueryData<InfiniteData<MessagesResponse>>(["messages", fromFolder]);
        if (prev) {
          const allMessages = prev.pages.flatMap((p) => p.messages);
          const idx = allMessages.findIndex((m) => m.uid === uid);
          const nextMsg = allMessages[idx + 1] ?? allMessages[idx - 1] ?? null;
          selectMessage(nextMsg?.uid ?? null);
        } else {
          selectMessage(null);
        }
      }

      // Cancel in-flight fetches so they don't overwrite our optimistic update.
      await Promise.all([
        queryClient.cancelQueries({ queryKey: ["messages", fromFolder] }),
        queryClient.cancelQueries({ queryKey: ["messages", toFolder] })
      ]);

      const prevFrom = queryClient.getQueryData<InfiniteData<MessagesResponse>>(
        ["messages", fromFolder],
      );
      const prevTo = queryClient.getQueryData<InfiniteData<MessagesResponse>>(
        ["messages", toFolder],
      );

      // Find the message in the source folder cache.
      let movedMsg: MessageHeader | undefined;
      if (prevFrom) {
        for (const page of prevFrom.pages) {
          movedMsg = page.messages.find((m) => m.uid === uid);
          if (movedMsg) break;
        }
      }

      // Remove from source folder cache.
      if (prevFrom) {
        queryClient.setQueryData<InfiniteData<MessagesResponse>>(
          ["messages", fromFolder],
          {
            ...prevFrom,
            pages: prevFrom.pages.map((page) => ({
              ...page,
              messages: page.messages.filter((m) => m.uid !== uid),
              total_count: Math.max(0, page.total_count - 1),
            })),
          },
        );
      }

      // Insert into destination folder cache (first page) with a placeholder
      // UID. The background refetch will reconcile with the real UID.
      if (movedMsg && prevTo) {
        const entry: MessageHeader = {
          ...movedMsg,
          folder: toFolder,
        };
        queryClient.setQueryData<InfiniteData<MessagesResponse>>(
          ["messages", toFolder],
          {
            ...prevTo,
            pages: prevTo.pages.map((page, i) =>
              i === 0
                ? {
                  ...page,
                  messages: [entry, ...page.messages],
                  total_count: page.total_count + 1,
                }
                : { ...page, total_count: page.total_count + 1 },
            ),
          },
        );
      }

      return { prevFrom, prevTo };
    },
    onError: (_err, { fromFolder, toFolder }, context) => {
      // Rollback on failure.
      if (context?.prevFrom) {
        queryClient.setQueryData(["messages", fromFolder], context.prevFrom);
      }
      if (context?.prevTo) {
        queryClient.setQueryData(["messages", toFolder], context.prevTo);
      }
    },
    onSettled: (_data, _err, { fromFolder, toFolder }) => {
      // Always refetch to reconcile with server state.
      queryClient.invalidateQueries({ queryKey: ["messages", fromFolder] });
      queryClient.invalidateQueries({ queryKey: ["messages", toFolder] });
      queryClient.invalidateQueries({ queryKey: ["folders"] });
    },
  });
}

export function useDeleteMessage() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ folder, uid }: { folder: string; uid: number }) =>
      apiDelete(`/messages/${encodeURIComponent(folder)}/${uid}`),
    onMutate: async ({ folder, uid }) => {
      // Auto-advance: if the deleted message is selected, select the next (or previous) message.
      const { selectedMessageUid, selectMessage } = useUiStore.getState();
      if (selectedMessageUid === uid) {
        const prev = queryClient.getQueryData<InfiniteData<MessagesResponse>>(["messages", folder]);
        if (prev) {
          const allMessages = prev.pages.flatMap((p) => p.messages);
          const idx = allMessages.findIndex((m) => m.uid === uid);
          const nextMsg = allMessages[idx + 1] ?? allMessages[idx - 1] ?? null;
          selectMessage(nextMsg?.uid ?? null);
        } else {
          selectMessage(null);
        }
      }

      // Optimistic removal from cache.
      await queryClient.cancelQueries({ queryKey: ["messages", folder] });
      const prev = queryClient.getQueryData<InfiniteData<MessagesResponse>>(
        ["messages", folder],
      );
      if (prev) {
        queryClient.setQueryData<InfiniteData<MessagesResponse>>(
          ["messages", folder],
          {
            ...prev,
            pages: prev.pages.map((page) => ({
              ...page,
              messages: page.messages.filter((m) => m.uid !== uid),
              total_count: Math.max(0, page.total_count - 1),
            })),
          },
        );
      }
      return { prev };
    },
    onError: (_err, { folder }, context) => {
      if (context?.prev) {
        queryClient.setQueryData(["messages", folder], context.prev);
      }
    },
    onSettled: (_, _err, { folder }) => {
      queryClient.invalidateQueries({ queryKey: ["messages", folder] });
      queryClient.invalidateQueries({ queryKey: ["folders"] });
    },
  });
}

/**
 * Prefetch the first page of messages for each folder in the background.
 * This triggers the backend to sync messages from IMAP lazily so folder
 * counts are populated and messages are ready when the user clicks a folder.
 */
export function usePrefetchAllFolders(folderNames: string[], activeFolder: string) {
  const queryClient = useQueryClient();
  const prefetched = useRef(false);

  useEffect(() => {
    if (prefetched.current || folderNames.length === 0) return;
    prefetched.current = true;

    // Prefetch each folder except the active one (already loaded by MessageList).
    for (const name of folderNames) {
      if (name === activeFolder) continue;
      queryClient.prefetchInfiniteQuery({
        queryKey: ["messages", name],
        queryFn: () =>
          apiGet<MessagesResponse>(
            `/folders/${encodeURIComponent(name)}/messages?page=0&per_page=${PER_PAGE}`,
          ),
        initialPageParam: 0,
      });
    }

    // Folder counts are updated by WebSocket events and background sync —
    // no need for a timer-based invalidation.
  }, [folderNames, activeFolder, queryClient]);
}
