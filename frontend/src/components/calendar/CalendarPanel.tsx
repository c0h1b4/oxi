"use client";

import { useEffect } from "react";
import { useCalendarStore } from "@/stores/useCalendarStore";
import { useCalendarSettings } from "@/hooks/useCalendar";
import { CalendarHeader } from "./CalendarHeader";
import { MonthView } from "./MonthView";
import { WeekView } from "./WeekView";
import { DayView } from "./DayView";
import { EventForm } from "./EventForm";
import { EventDetail } from "./EventDetail";

export function CalendarPanel() {
  const viewMode = useCalendarStore((s) => s.viewMode);
  const setViewMode = useCalendarStore((s) => s.setViewMode);
  const { data: settings } = useCalendarSettings();

  const weekStartsOn = settings?.week_starts_on ?? 0;
  const timeFormat = settings?.time_format ?? "12h";

  // Apply default view from settings on first load
  useEffect(() => {
    if (settings?.default_view) {
      const v = settings.default_view as "month" | "week" | "day";
      if (["month", "week", "day"].includes(v)) {
        setViewMode(v);
      }
    }
  }, [settings?.default_view]); // eslint-disable-line react-hooks/exhaustive-deps -- only apply on initial settings load

  return (
    <div className="flex h-full min-w-0 flex-1 flex-col">
      <CalendarHeader />

      <div className="flex flex-1 overflow-hidden">
        {viewMode === "month" && (
          <MonthView weekStartsOn={weekStartsOn} timeFormat={timeFormat} />
        )}
        {viewMode === "week" && (
          <WeekView weekStartsOn={weekStartsOn} timeFormat={timeFormat} />
        )}
        {viewMode === "day" && <DayView timeFormat={timeFormat} />}
      </div>

      <EventForm />
      <EventDetail timeFormat={timeFormat} />
    </div>
  );
}
