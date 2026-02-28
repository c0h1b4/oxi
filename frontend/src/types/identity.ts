export interface Identity {
  id: number;
  display_name: string;
  email: string;
  signature_html: string;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateIdentityRequest {
  email: string;
  display_name?: string;
  signature_html?: string;
  is_default?: boolean;
}

export interface UpdateIdentityRequest {
  email?: string;
  display_name?: string;
  signature_html?: string;
  is_default?: boolean;
}
