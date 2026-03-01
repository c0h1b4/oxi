"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPost, apiPatch, apiDelete } from "@/lib/api";
import { useWsStatus } from "@/lib/ws-context";
import type { FoldersResponse } from "@/types/folder";

export function useFolders() {
  const { status } = useWsStatus();
  return useQuery({
    queryKey: ["folders"],
    queryFn: () => apiGet<FoldersResponse>("/folders"),
    refetchInterval: status === "connected" ? false : 30_000,
  });
}

export function useCreateFolder() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ name }: { name: string }) =>
      apiPost("/folders", { name }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["folders"] }),
  });
}

export function useRenameFolder() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ name, newName }: { name: string; newName: string }) =>
      apiPatch(`/folders/${encodeURIComponent(name)}`, { new_name: newName }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["folders"] }),
  });
}

export function useDeleteFolder() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ name }: { name: string }) =>
      apiDelete(`/folders/${encodeURIComponent(name)}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["folders"] }),
  });
}
