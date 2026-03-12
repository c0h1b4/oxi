"use client";

import { create } from "zustand";

interface CalendarState {
  selectedDate: Date;
  viewMode: "month" | "week" | "day";
  selectedEvent: string | null;
  showEventForm: boolean;
  editingEventId: string | null;

  setDate: (date: Date) => void;
  setViewMode: (mode: "month" | "week" | "day") => void;
  selectEvent: (id: string | null) => void;
  openEventForm: (editId?: string) => void;
  closeEventForm: () => void;
}

export const useCalendarStore = create<CalendarState>((set) => ({
  selectedDate: new Date(),
  viewMode: "month",
  selectedEvent: null,
  showEventForm: false,
  editingEventId: null,

  setDate: (date) => set({ selectedDate: date }),
  setViewMode: (mode) => set({ viewMode: mode }),
  selectEvent: (id) => set({ selectedEvent: id }),
  openEventForm: (editId) =>
    set({
      showEventForm: true,
      editingEventId: editId ?? null,
      selectedEvent: null,
    }),
  closeEventForm: () =>
    set({ showEventForm: false, editingEventId: null }),
}));
