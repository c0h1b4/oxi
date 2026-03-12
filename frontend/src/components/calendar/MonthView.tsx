"use client";

import { useMemo } from "react";
import { useCalendarStore } from "@/stores/useCalendarStore";
import { useCalendarEvents } from "@/hooks/useCalendar";
import {
  getMonthGrid,
  getDayNames,
  getMonthViewRange,
  isSameDay,
  isToday,
  getEventColorClasses,
  formatTime,
} from "./calendarUtils";
import { cn } from "@/lib/utils";
import type { CalendarEvent } from "@/types/calendar";

interface MonthViewProps {
  weekStartsOn: number;
  timeFormat: string;
}

export function MonthView({ weekStartsOn, timeFormat }: MonthViewProps) {
  const selectedDate = useCalendarStore((s) => s.selectedDate);
  const setDate = useCalendarStore((s) => s.setDate);
  const setViewMode = useCalendarStore((s) => s.setViewMode);
  const selectEvent = useCalendarStore((s) => s.selectEvent);
  const openEventForm = useCalendarStore((s) => s.openEventForm);

  const year = selectedDate.getFullYear();
  const month = selectedDate.getMonth();

  const range = useMemo(
    () => getMonthViewRange(year, month, weekStartsOn),
    [year, month, weekStartsOn],
  );
  const { data } = useCalendarEvents(range.start, range.end);
  const events = useMemo(() => data?.events ?? [], [data]);

  const days = useMemo(
    () => getMonthGrid(year, month, weekStartsOn),
    [year, month, weekStartsOn],
  );
  const dayNames = useMemo(() => getDayNames(weekStartsOn), [weekStartsOn]);

  const eventsByDay = useMemo(() => {
    const map = new Map<string, CalendarEvent[]>();
    for (const event of events) {
      const start = new Date(event.start_time);
      const end = new Date(event.end_time);

      // For multi-day / all-day events, add to each day they span
      const current = new Date(start);
      if (event.all_day) {
        current.setHours(0, 0, 0, 0);
      }
      const endDate = new Date(end);
      while (current <= endDate) {
        const key = `${current.getFullYear()}-${current.getMonth()}-${current.getDate()}`;
        if (!map.has(key)) map.set(key, []);
        map.get(key)!.push(event);
        current.setDate(current.getDate() + 1);
        current.setHours(0, 0, 0, 0);
      }
    }
    return map;
  }, [events]);

  const handleDayClick = (day: Date) => {
    setDate(day);
  };

  const handleDayDoubleClick = (day: Date) => {
    setDate(day);
    openEventForm();
  };

  const handleEventClick = (e: React.MouseEvent, eventId: string) => {
    e.stopPropagation();
    selectEvent(eventId);
  };

  const handleDayLabelClick = (e: React.MouseEvent, day: Date) => {
    e.stopPropagation();
    setDate(day);
    setViewMode("day");
  };

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Day headers */}
      <div className="grid grid-cols-7 border-b border-border">
        {dayNames.map((name) => (
          <div
            key={name}
            className="px-2 py-1.5 text-center text-xs font-medium text-muted-foreground"
          >
            {name}
          </div>
        ))}
      </div>

      {/* Day grid */}
      <div className="grid flex-1 grid-cols-7 grid-rows-6">
        {days.map((day, idx) => {
          const dayKey = `${day.getFullYear()}-${day.getMonth()}-${day.getDate()}`;
          const dayEvents = eventsByDay.get(dayKey) ?? [];
          const isCurrentMonth = day.getMonth() === month;
          const isSelected = isSameDay(day, selectedDate);
          const isTodayDate = isToday(day);

          return (
            <div
              key={idx}
              onClick={() => handleDayClick(day)}
              onDoubleClick={() => handleDayDoubleClick(day)}
              className={cn(
                "min-h-0 cursor-pointer border-b border-r border-border p-1 transition-colors hover:bg-accent/50",
                !isCurrentMonth && "bg-muted/30",
                isSelected && "bg-accent",
              )}
            >
              <button
                type="button"
                onClick={(e) => handleDayLabelClick(e, day)}
                className={cn(
                  "mb-0.5 flex size-6 items-center justify-center rounded-full text-xs transition-colors hover:bg-primary hover:text-primary-foreground",
                  isTodayDate && "bg-primary text-primary-foreground font-bold",
                  !isCurrentMonth && "text-muted-foreground/50",
                )}
              >
                {day.getDate()}
              </button>

              {/* Event chips */}
              <div className="space-y-0.5 overflow-hidden">
                {dayEvents.slice(0, 3).map((event) => {
                  const colors = getEventColorClasses(event.color);
                  return (
                    <button
                      key={event.id}
                      type="button"
                      onClick={(e) => handleEventClick(e, event.id)}
                      className={cn(
                        "block w-full truncate rounded px-1 py-0.5 text-left text-[10px] leading-tight",
                        colors.bg,
                        colors.text,
                      )}
                      title={event.title}
                    >
                      {!event.all_day && (
                        <span className="mr-0.5 opacity-80">
                          {formatTime(event.start_time, timeFormat)}
                        </span>
                      )}
                      {event.title}
                    </button>
                  );
                })}
                {dayEvents.length > 3 && (
                  <div className="text-[10px] text-muted-foreground">
                    +{dayEvents.length - 3} more
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
