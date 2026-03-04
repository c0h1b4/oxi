"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPost, apiPut, apiDelete } from "@/lib/api";
import type { Tag, MessageTag } from "@/types/tag";
import type { MessageHeader } from "@/types/message";

interface ListTagsResponse {
  tags: Tag[];
}

interface MessageTagsResponse {
  tags: MessageTag[];
}

interface TagMessagesResponse {
  messages: (MessageHeader & { tags: MessageTag[] })[];
  total_count: number;
  page: number;
  per_page: number;
}

export function useTags() {
  return useQuery({
    queryKey: ["tags"],
    queryFn: () => apiGet<ListTagsResponse>("/tags"),
  });
}

export function useCreateTag() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ name, color }: { name: string; color?: string }) =>
      apiPost<{ id: string; name: string; color: string }>("/tags", {
        name,
        ...(color ? { color } : {}),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["tags"] });
    },
  });
}

export function useUpdateTag() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      id,
      name,
      color,
    }: {
      id: string;
      name: string;
      color: string;
    }) => apiPut<{ id: string; name: string; color: string }>(`/tags/${id}`, { name, color }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["tags"] });
    },
  });
}

export function useDeleteTag() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiDelete(`/tags/${id}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["tags"] });
      queryClient.invalidateQueries({ queryKey: ["message-tags"] });
    },
  });
}

export function useMessageTags(folder: string | null, uid: number | null) {
  return useQuery({
    queryKey: ["message-tags", folder, uid],
    queryFn: () =>
      apiGet<MessageTagsResponse>(
        `/messages/${encodeURIComponent(folder!)}/${uid}/tags`,
      ),
    enabled: !!folder && uid != null && uid > 0,
  });
}

export function useAddTagToMessage() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      tagId,
      messageUid,
      messageFolder,
    }: {
      tagId: string;
      messageUid: number;
      messageFolder: string;
    }) =>
      apiPost(`/tags/${tagId}/messages`, {
        message_uid: messageUid,
        message_folder: messageFolder,
      }),
    onSuccess: (_, { tagId, messageUid, messageFolder }) => {
      queryClient.invalidateQueries({
        queryKey: ["message-tags", messageFolder, messageUid],
      });
      queryClient.invalidateQueries({ queryKey: ["tags"] });
      queryClient.invalidateQueries({ queryKey: ["tag-messages", tagId] });
      queryClient.invalidateQueries({ queryKey: ["messages", messageFolder] });
    },
  });
}

export function useRemoveTagFromMessage() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      tagId,
      messageUid,
      messageFolder,
    }: {
      tagId: string;
      messageUid: number;
      messageFolder: string;
    }) =>
      apiDelete(
        `/tags/${tagId}/messages/${encodeURIComponent(messageFolder)}/${messageUid}`,
      ),
    onSuccess: (_, { tagId, messageUid, messageFolder }) => {
      queryClient.invalidateQueries({
        queryKey: ["message-tags", messageFolder, messageUid],
      });
      queryClient.invalidateQueries({ queryKey: ["tags"] });
      queryClient.invalidateQueries({ queryKey: ["tag-messages", tagId] });
      queryClient.invalidateQueries({ queryKey: ["messages", messageFolder] });
    },
  });
}

export function useBulkAddTag() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      tagId,
      messages,
    }: {
      tagId: string;
      messages: { uid: number; folder: string }[];
    }) =>
      apiPost(`/tags/${tagId}/messages/bulk`, { messages }),
    onSuccess: (_, { tagId, messages }) => {
      queryClient.invalidateQueries({ queryKey: ["tags"] });
      queryClient.invalidateQueries({ queryKey: ["tag-messages", tagId] });
      // Invalidate message-tags and message list for each affected folder.
      const folders = new Set(messages.map((m) => m.folder));
      for (const folder of folders) {
        queryClient.invalidateQueries({ queryKey: ["messages", folder] });
      }
      queryClient.invalidateQueries({ queryKey: ["message-tags"] });
    },
  });
}

export function useTagMessages(tagId: string | null) {
  return useQuery({
    queryKey: ["tag-messages", tagId],
    queryFn: () =>
      apiGet<TagMessagesResponse>(`/tags/${tagId}/messages?per_page=200`),
    enabled: !!tagId,
  });
}
