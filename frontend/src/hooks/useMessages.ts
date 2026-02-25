"use client";

import { useEffect, useRef } from "react";
import {
  useQuery,
  useInfiniteQuery,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";
import { apiGet, apiPatch, apiPost, apiDelete } from "@/lib/api";
import type { MessagesResponse, MessageDetail } from "@/types/message";

const PER_PAGE = 50;

export function useMessages(folder: string) {
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
    refetchInterval: 60_000, // Poll every 60 seconds for new/changed messages
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
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["messages"] });
      queryClient.invalidateQueries({ queryKey: ["folders"] });
    },
  });
}

export function useDeleteMessage() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ folder, uid }: { folder: string; uid: number }) =>
      apiDelete(`/messages/${encodeURIComponent(folder)}/${uid}`),
    onSuccess: (_, { folder }) => {
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

    // Re-fetch folders after a delay so updated counts propagate.
    setTimeout(() => {
      queryClient.invalidateQueries({ queryKey: ["folders"] });
    }, 5000);
  }, [folderNames, activeFolder, queryClient]);
}
