"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { apiPost } from "@/lib/api";

interface SendParams {
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  body: string;
  inReplyTo: string | null;
  references: string | null;
}

interface SendResponse {
  status: string;
  message_id: string;
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
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["messages"] });
      queryClient.invalidateQueries({ queryKey: ["folders"] });
    },
  });
}
