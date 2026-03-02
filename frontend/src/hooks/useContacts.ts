"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPost, apiDelete } from "@/lib/api";
import type { Contact, ContactsResponse } from "@/types/contact";

export function useContacts(search?: string) {
  const params = new URLSearchParams();
  if (search) params.set("q", search);
  params.set("limit", "50");
  params.set("offset", "0");

  return useQuery({
    queryKey: ["contacts", search ?? ""],
    queryFn: () => apiGet<ContactsResponse>(`/contacts?${params.toString()}`),
  });
}

export function useCreateContact() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: {
      email: string;
      name: string;
      company?: string;
      notes?: string;
      is_favorite?: boolean;
    }) => apiPost<Contact>("/contacts", body as Record<string, unknown>),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["contacts"] });
    },
  });
}

export function useDeleteContact() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiDelete(`/contacts/${id}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["contacts"] });
    },
  });
}

export function useAutocomplete(query: string) {
  return useQuery({
    queryKey: ["contacts-autocomplete", query],
    queryFn: async () => {
      const res = await apiGet<{ suggestions: { email: string; name: string }[] }>(
        `/contacts/autocomplete?q=${encodeURIComponent(query)}&limit=10`,
      );
      return res.suggestions;
    },
    enabled: query.length >= 2,
  });
}
