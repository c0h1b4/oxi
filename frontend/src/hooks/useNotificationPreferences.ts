"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPut } from "@/lib/api";

export interface NotificationPreferences {
  enabled: boolean;
  sound: boolean;
  folders: string[];
  updated_at: string;
}

interface UpdateNotificationPreferences {
  enabled?: boolean;
  sound?: boolean;
  folders?: string[];
}

export function useNotificationPreferences() {
  return useQuery({
    queryKey: ["notification-preferences"],
    queryFn: () => apiGet<NotificationPreferences>("/settings/notifications"),
  });
}

export function useUpdateNotificationPreferences() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: UpdateNotificationPreferences) =>
      apiPut<NotificationPreferences>(
        "/settings/notifications",
        data as Record<string, unknown>,
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["notification-preferences"] });
    },
  });
}
