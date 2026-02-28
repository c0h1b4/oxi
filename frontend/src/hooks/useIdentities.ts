"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPost, apiPut, apiDelete } from "@/lib/api";
import type {
  Identity,
  CreateIdentityRequest,
  UpdateIdentityRequest,
} from "@/types/identity";

export function useIdentities() {
  return useQuery({
    queryKey: ["identities"],
    queryFn: () => apiGet<Identity[]>("/identities"),
  });
}

export function useCreateIdentity() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: CreateIdentityRequest) =>
      apiPost<Identity>("/identities", data as unknown as Record<string, unknown>),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
    },
  });
}

export function useUpdateIdentity() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpdateIdentityRequest }) =>
      apiPut<Identity>(`/identities/${id}`, data as unknown as Record<string, unknown>),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
    },
  });
}

export function useDeleteIdentity() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiDelete<{ status: string }>(`/identities/${id}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
    },
  });
}
