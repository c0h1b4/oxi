"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { apiPost, apiPostFormData, apiDelete } from "@/lib/api";

interface SendParams {
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  body: string;
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
        html_body: null,
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
