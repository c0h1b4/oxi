"use client";

import { useQuery } from "@tanstack/react-query";
import { apiGet } from "@/lib/api";
import { useAuthStore } from "@/stores/useAuthStore";

interface QuotaResponse {
  usage_bytes: number | null;
  limit_bytes: number | null;
}

export function useQuota() {
  const activeAccountId = useAuthStore((s) => s.activeAccountId);
  return useQuery({
    queryKey: ["quota", activeAccountId],
    queryFn: () => apiGet<QuotaResponse>("/quota"),
    enabled: !!activeAccountId,
    staleTime: 5 * 60 * 1000, // 5 minutes
    refetchInterval: 5 * 60 * 1000,
  });
}
