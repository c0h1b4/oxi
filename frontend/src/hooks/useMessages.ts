"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPatch, apiPost, apiDelete } from "@/lib/api";
import type { MessagesResponse, MessageDetail } from "@/types/message";

export function useMessages(
  folder: string,
  page: number = 0,
  perPage: number = 50,
) {
  return useQuery({
    queryKey: ["messages", folder, page],
    queryFn: () =>
      apiGet<MessagesResponse>(
        `/folders/${encodeURIComponent(folder)}/messages?page=${page}&per_page=${perPage}`,
      ),
    enabled: !!folder,
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
    onSuccess: (_, { folder }) => {
      queryClient.invalidateQueries({ queryKey: ["messages", folder] });
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
