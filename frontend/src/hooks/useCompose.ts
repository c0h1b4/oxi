"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { apiPost, apiGet, apiPostFormData, apiDelete } from "@/lib/api";

interface SendParams {
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  body: string;
  htmlBody: string | null;
  inReplyTo: string | null;
  references: string | null;
  draftId: string | null;
}

interface SendResponse {
  status: string;
  message_id: string;
}

interface UploadResponse {
  attachments: {
    id: string;
    filename: string;
    content_type: string;
    size: number;
  }[];
}

interface DeleteAttachmentResponse {
  status: string;
}

interface SaveDraftParams {
  id: string;
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  textBody: string;
  htmlBody: string | null;
  inReplyTo: string | null;
  references: string | null;
}

interface SaveDraftResponse {
  id: string;
  status: string;
}

interface DraftListItem {
  id: string;
  to: string;
  subject: string;
  updated_at: string;
}

interface DraftListResponse {
  drafts: DraftListItem[];
}

interface DraftDetail {
  id: string;
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  text_body: string;
  html_body: string | null;
  in_reply_to: string | null;
  references: string | null;
  created_at: string;
  updated_at: string;
  attachments: {
    id: string;
    filename: string;
    content_type: string;
    size: number;
  }[];
}

function parseRecipients(raw: string): string[] {
  return raw
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

export function useSendMessage() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: SendParams) =>
      apiPost<SendResponse>("/messages/send", {
        to: parseRecipients(params.to),
        cc: parseRecipients(params.cc),
        bcc: parseRecipients(params.bcc),
        subject: params.subject,
        text_body: params.body,
        html_body: params.htmlBody,
        in_reply_to: params.inReplyTo,
        references: params.references,
        draft_id: params.draftId,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["messages"] });
      queryClient.invalidateQueries({ queryKey: ["folders"] });
    },
  });
}

export function useUploadAttachment() {
  return useMutation({
    mutationFn: ({
      draftId,
      files,
    }: {
      draftId: string;
      files: File[];
    }) => {
      const formData = new FormData();
      for (const file of files) {
        formData.append("file", file);
      }
      return apiPostFormData<UploadResponse>(
        `/drafts/${draftId}/attachments`,
        formData,
      );
    },
  });
}

export function useDeleteAttachment() {
  return useMutation({
    mutationFn: ({
      draftId,
      attachmentId,
    }: {
      draftId: string;
      attachmentId: string;
    }) =>
      apiDelete<DeleteAttachmentResponse>(
        `/drafts/${draftId}/attachments/${attachmentId}`,
      ),
  });
}

export function useSaveDraft() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: SaveDraftParams) =>
      apiPost<SaveDraftResponse>("/drafts", {
        id: params.id,
        to: params.to,
        cc: params.cc,
        bcc: params.bcc,
        subject: params.subject,
        text_body: params.textBody,
        html_body: params.htmlBody,
        in_reply_to: params.inReplyTo,
        references: params.references,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["drafts"] });
    },
  });
}

export function useListDrafts(enabled: boolean) {
  return useQuery({
    queryKey: ["drafts"],
    queryFn: () => apiGet<DraftListResponse>("/drafts"),
    enabled,
  });
}

export function useGetDraft(id: string | null) {
  return useQuery({
    queryKey: ["drafts", id],
    queryFn: () => apiGet<DraftDetail>(`/drafts/${id}`),
    enabled: !!id,
  });
}

export function useDeleteDraft() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) =>
      apiDelete<{ status: string }>(`/drafts/${id}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["drafts"] });
    },
  });
}
