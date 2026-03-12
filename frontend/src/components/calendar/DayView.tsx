"use client";

import { useMemo, useRef, useEffect } from "react";
import { useCalendarStore } from "@/stores/useCalendarStore";
import { useCalendarEvents } from "@/hooks/useCalendar";
import {
  getDayViewRange,
  getHours,
  getEventPosition,
  isToday,
  formatTime,
  getEventColorClasses,
} from "./calendarUtils";
import { cn } from "@/lib/utils";
import type { CalendarEvent } from "@/types/calendar";

interface DayViewProps {
  timeFormat: string;
}

export function DayView({ timeFormat }: DayViewProps) {
  const selectedDate = useCalendarStore((s) => s.selectedDate);
  const setDate = useCalendarStore((s) => s.setDate);
  const selectEvent = useCalendarStore((s) => s.selectEvent);
  const openEventForm = useCalendarStore((s) => s.openEventForm);
  const scrollRef = useRef<HTMLDivElement>(null);

  const range = useMemo(
    () => getDayViewRange(selectedDate),
    [selectedDate],
  );
  const { data } = useCalendarEvents(range.start, range.end);
  const events = useMemo(() => data?.events ?? [], [data]);

  const hours = getHours();

  // Scroll to 8am on mount
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = 8 * 60;
    }
  }, []);

  const { allDayEvents, timedEvents } = useMemo(() => {
    const allDay: CalendarEvent[] = [];
    const timed: CalendarEvent[] = [];

    for (const event of events) {
      if (event.all_day) {
        allDay.push(event);
      } else {
        timed.push(event);
      }
    }

    return { allDayEvents: allDay, timedEvents: timed };
  }, [events]);

  const handleTimeSlotClick = (hour: number) => {
    const d = new Date(selectedDate);
    d.setHours(hour, 0, 0, 0);
    setDate(d);
    openEventForm();
  };

  const handleEventClick = (e: React.MouseEvent, eventId: string) => {
    e.stopPropagation();
    selectEvent(eventId);
  };

  const now = new Date();
  const currentMinutes = now.getHours() * 60 + now.getMinutes();
  const showIndicator = isToday(selectedDate);

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* All-day events */}
      {allDayEvents.length > 0 && (
        <div className="flex border-b border-border">
          <div className="w-[60px] shrink-0 border-r border-border px-1 py-1 text-[10px] text-muted-foreground">
            All day
          </div>
          <div className="flex-1 p-1 space-y-0.5">
            {allDayEvents.map((ev) => {
              const colors = getEventColorClasses(ev.color);
              return (
                <button
                  key={ev.id}
                  type="button"
                  onClick={(e) => handleEventClick(e, ev.id)}
                  className={cn(
                    "block w-full truncate rounded px-2 py-1 text-left text-xs",
                    colors.bg,
                    colors.text,
                  )}
                >
                  {ev.title}
                </button>
              );
            })}
          </div>
        </div>
      )}

      {/* Time grid */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto">
        <div className="relative flex">
          {/* Time labels */}
          <div className="w-[60px] shrink-0 border-r border-border">
            {hours.map((hour) => (
              <div
                key={hour}
                className="relative h-[60px] border-b border-border pr-2 text-right"
              >
                <span className="absolute -top-2 right-2 text-[10px] text-muted-foreground">
                  {formatTime(
                    `2025-01-01T${String(hour).padStart(2, "0")}:00:00`,
                    timeFormat,
                  )}
                </span>
              </div>
            ))}
          </div>

          {/* Event area */}
          <div className="relative flex-1">
            {/* Hour slots */}
            {hours.map((hour) => (
              <div
                key={hour}
                onClick={() => handleTimeSlotClick(hour)}
                className="h-[60px] cursor-pointer border-b border-border transition-colors hover:bg-accent/30"
              />
            ))}

            {/* Current time indicator */}
            {showIndicator && (
              <div
                className="pointer-events-none absolute left-0 right-0 z-10 border-t-2 border-red-500"
                style={{ top: `${currentMinutes}px` }}
              >
                <div className="absolute -left-1 -top-1.5 size-3 rounded-full bg-red-500" />
              </div>
            )}

            {/* Events */}
            {timedEvents.map((event) => {
              const pos = getEventPosition(
                event.start_time,
                event.end_time,
                selectedDate,
              );
              const colors = getEventColorClasses(event.color);
              return (
                <button
                  key={event.id}
                  type="button"
                  onClick={(e) => handleEventClick(e, event.id)}
                  className={cn(
                    "absolute left-1 right-4 overflow-hidden rounded px-2 py-1 text-left text-xs leading-tight",
                    colors.bg,
                    colors.text,
                  )}
                  style={{
                    top: `${pos.top}px`,
                    height: `${pos.height}px`,
                  }}
                  title={event.title}
                >
                  <div className="font-medium">{event.title}</div>
                  <div className="opacity-80">
                    {formatTime(event.start_time, timeFormat)} -{" "}
                    {formatTime(event.end_time, timeFormat)}
                  </div>
                  {event.location && (
                    <div className="mt-0.5 opacity-70">{event.location}</div>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
