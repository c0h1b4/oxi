"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPost, apiPut, apiDelete } from "@/lib/api";
import type {
  CalendarEvent,
  CalendarEventsResponse,
  CalendarSettings,
  CreateEventRequest,
  UpdateEventRequest,
  MeetingTemplate,
  MeetingTemplatesResponse,
} from "@/types/calendar";

export function useCalendarEvents(start: string, end: string) {
  return useQuery({
    queryKey: ["calendar-events", start, end],
    queryFn: () =>
      apiGet<CalendarEventsResponse>(
        `/calendar/events?start=${encodeURIComponent(start)}&end=${encodeURIComponent(end)}`,
      ),
    enabled: !!start && !!end,
  });
}

export function useCalendarEvent(id: string | null) {
  return useQuery({
    queryKey: ["calendar-event", id],
    queryFn: () => apiGet<CalendarEvent>(`/calendar/events/${id}`),
    enabled: !!id,
  });
}

export function useCreateEvent() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateEventRequest) =>
      apiPost<CalendarEvent>(
        "/calendar/events",
        body as unknown as Record<string, unknown>,
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["calendar-events"] });
    },
  });
}

export function useUpdateEvent() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...body }: UpdateEventRequest & { id: string }) =>
      apiPut<CalendarEvent>(
        `/calendar/events/${id}`,
        body as unknown as Record<string, unknown>,
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["calendar-events"] });
      queryClient.invalidateQueries({ queryKey: ["calendar-event"] });
    },
  });
}

export function useDeleteEvent() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiDelete(`/calendar/events/${id}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["calendar-events"] });
    },
  });
}

export function useCalendarSettings() {
  return useQuery({
    queryKey: ["calendar-settings"],
    queryFn: () => apiGet<CalendarSettings>("/calendar/settings"),
  });
}

export function useUpdateCalendarSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: Partial<CalendarSettings>) =>
      apiPut<CalendarSettings>(
        "/calendar/settings",
        body as Record<string, unknown>,
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["calendar-settings"] });
    },
  });
}

export function useMeetingTemplates() {
  return useQuery({
    queryKey: ["meeting-templates"],
    queryFn: () =>
      apiGet<MeetingTemplatesResponse>("/calendar/meeting-templates"),
  });
}

export function useCreateMeetingTemplate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: {
      name: string;
      url_template: string;
      icon?: string;
    }) =>
      apiPost<MeetingTemplate>(
        "/calendar/meeting-templates",
        body as Record<string, unknown>,
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["meeting-templates"] });
    },
  });
}

export function useDeleteMeetingTemplate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiDelete(`/calendar/meeting-templates/${id}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["meeting-templates"] });
    },
  });
}
