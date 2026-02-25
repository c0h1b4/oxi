export interface Folder {
  name: string;
  delimiter: string | null;
  attributes: string[];
  is_subscribed: boolean;
  total_count: number;
  unread_count: number;
}

export interface FoldersResponse {
  folders: Folder[];
}
