"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPut } from "@/lib/api";
import type { AnimationMode } from "@/lib/motion/config";

export interface DisplayPreferences {
  density: "compact" | "comfortable";
  theme: "light" | "dark" | "system";
  language: string;
  compose_format: "html" | "text";
  animation_mode?: AnimationMode | null;
  deep_index: boolean;
  updated_at: string;
}

interface UpdateDisplayPreferences {
  density?: "compact" | "comfortable";
  theme?: "light" | "dark" | "system";
  language?: string;
  compose_format?: "html" | "text";
  animation_mode?: AnimationMode | null;
  deep_index?: boolean;
}

export function useDisplayPreferences() {
  return useQuery({
    queryKey: ["display-preferences"],
    queryFn: () => apiGet<DisplayPreferences>("/settings/display"),
  });
}

export function useUpdateDisplayPreferences() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: UpdateDisplayPreferences) =>
      apiPut<DisplayPreferences>(
        "/settings/display",
        data as Record<string, unknown>,
      ),
    onSuccess: (result) => {
      queryClient.setQueryData(["display-preferences"], result);
    },
  });
}
