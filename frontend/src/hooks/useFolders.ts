"use client";

import { useQuery } from "@tanstack/react-query";
import { apiGet } from "@/lib/api";
import type { FoldersResponse } from "@/types/folder";

export function useFolders() {
  return useQuery({
    queryKey: ["folders"],
    queryFn: () => apiGet<FoldersResponse>("/folders"),
    refetchInterval: 30_000, // Poll every 30 seconds for folder count updates
  });
}
