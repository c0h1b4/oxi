import type { MessageTag } from "@/types/tag";

export interface EmailAddress {
  name: string | null;
  address: string;
}

export interface MessageHeader {
  uid: number;
  folder: string;
  message_id: string | null;
  in_reply_to: string | null;
  subject: string;
  from_address: string;
  from_name: string;
  to_addresses: string;
  cc_addresses: string;
  date: string;
  flags: string;
  size: number;
  has_attachments: boolean;
  snippet: string;
  reaction: string | null;
  tags: MessageTag[];
}

export interface MessageDetail {
  uid: number;
  folder: string;
  subject: string;
  from_address: string;
  from_name: string;
  to_addresses: EmailAddress[];
  cc_addresses: EmailAddress[];
  date: string;
  flags: string[];
  html: string | null;
  text: string | null;
  raw_headers: string;
  attachments: Attachment[];
  thread: MessageHeader[];
}

export interface Attachment {
  id: string;
  filename: string | null;
  content_type: string;
  size: number;
  content_id: string | null;
}

export interface MessagesResponse {
  messages: MessageHeader[];
  total_count: number;
  page: number;
  per_page: number;
  syncing?: boolean;
}

export interface SearchResultItem {
  uid: number;
  folder: string;
  score: number;
  subject: string;
  from_address: string;
  from_name: string;
  to_addresses: string;
  date: string;
  flags: string;
  has_attachments: boolean;
  snippet: string;
}

export interface SearchResponse {
  results: SearchResultItem[];
  total_count: number;
  query: string;
}
