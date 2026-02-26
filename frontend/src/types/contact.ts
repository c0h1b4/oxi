export interface Contact {
  id: string;
  email: string;
  name: string;
  company: string;
  notes: string;
  is_favorite: boolean;
  last_contacted: string | null;
  contact_count: number;
  source: string;
  created_at: string;
  updated_at: string;
}

export interface ContactsResponse {
  contacts: Contact[];
  total: number;
}
