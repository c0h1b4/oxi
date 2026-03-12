"use client";

import { useMemo, useRef, useEffect } from "react";
import { useCalendarStore } from "@/stores/useCalendarStore";
import { useCalendarEvents } from "@/hooks/useCalendar";
import {
  getWeekDays,
  getWeekViewRange,
  getHours,
  getEventPosition,
  isSameDay,
  isToday,
  formatTime,
  getEventColorClasses,
} from "./calendarUtils";
import { cn } from "@/lib/utils";
import type { CalendarEvent } from "@/types/calendar";

interface WeekViewProps {
  weekStartsOn: number;
  timeFormat: string;
}

export function WeekView({ weekStartsOn, timeFormat }: WeekViewProps) {
  const selectedDate = useCalendarStore((s) => s.selectedDate);
  const setDate = useCalendarStore((s) => s.setDate);
  const selectEvent = useCalendarStore((s) => s.selectEvent);
  const openEventForm = useCalendarStore((s) => s.openEventForm);
  const setViewMode = useCalendarStore((s) => s.setViewMode);
  const scrollRef = useRef<HTMLDivElement>(null);

  const days = useMemo(
    () => getWeekDays(selectedDate, weekStartsOn),
    [selectedDate, weekStartsOn],
  );

  const range = useMemo(
    () => getWeekViewRange(selectedDate, weekStartsOn),
    [selectedDate, weekStartsOn],
  );
  const { data } = useCalendarEvents(range.start, range.end);
  const events = useMemo(() => data?.events ?? [], [data]);

  const hours = getHours();

  // Scroll to 8am on mount
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = 8 * 60; // 8 hours * 60px/hour
    }
  }, []);

  // Categorize events into all-day and timed
  const { allDayEvents, timedEventsByDay } = useMemo(() => {
    const allDay: CalendarEvent[] = [];
    const timed = new Map<number, CalendarEvent[]>();

    for (let i = 0; i < 7; i++) {
      timed.set(i, []);
    }

    for (const event of events) {
      if (event.all_day) {
        allDay.push(event);
      } else {
        const startDate = new Date(event.start_time);
        for (let i = 0; i < 7; i++) {
          if (isSameDay(startDate, days[i])) {
            timed.get(i)!.push(event);
            break;
          }
        }
      }
    }

    return { allDayEvents: allDay, timedEventsByDay: timed };
  }, [events, days]);

  const handleTimeSlotClick = (day: Date, hour: number) => {
    const d = new Date(day);
    d.setHours(hour, 0, 0, 0);
    setDate(d);
    openEventForm();
  };

  const handleEventClick = (e: React.MouseEvent, eventId: string) => {
    e.stopPropagation();
    selectEvent(eventId);
  };

  // Current time indicator position
  const now = new Date();
  const currentMinutes = now.getHours() * 60 + now.getMinutes();

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Day headers */}
      <div className="grid grid-cols-[60px_repeat(7,1fr)] border-b border-border">
        <div className="border-r border-border" />
        {days.map((day, i) => (
          <button
            key={i}
            type="button"
            onClick={() => {
              setDate(day);
              setViewMode("day");
            }}
            className={cn(
              "border-r border-border px-2 py-2 text-center transition-colors hover:bg-accent",
              isToday(day) && "bg-primary/10",
            )}
          >
            <div className="text-xs text-muted-foreground">
              {day.toLocaleDateString("en-US", { weekday: "short" })}
            </div>
            <div
              className={cn(
                "text-lg font-medium",
                isToday(day) && "text-primary",
              )}
            >
              {day.getDate()}
            </div>
          </button>
        ))}
      </div>

      {/* All-day events row */}
      {allDayEvents.length > 0 && (
        <div className="grid grid-cols-[60px_repeat(7,1fr)] border-b border-border">
          <div className="border-r border-border px-1 py-1 text-[10px] text-muted-foreground">
            All day
          </div>
          {days.map((day, i) => {
            const dayAllDay = allDayEvents.filter((ev) => {
              const start = new Date(ev.start_time);
              const end = new Date(ev.end_time);
              return day >= start && day <= end;
            });
            return (
              <div key={i} className="border-r border-border p-0.5">
                {dayAllDay.map((ev) => {
                  const colors = getEventColorClasses(ev.color);
                  return (
                    <button
                      key={ev.id}
                      type="button"
                      onClick={(e) => handleEventClick(e, ev.id)}
                      className={cn(
                        "block w-full truncate rounded px-1 py-0.5 text-left text-[10px]",
                        colors.bg,
                        colors.text,
                      )}
                    >
                      {ev.title}
                    </button>
                  );
                })}
              </div>
            );
          })}
        </div>
      )}

      {/* Time grid */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto">
        <div className="relative grid grid-cols-[60px_repeat(7,1fr)]">
          {/* Time labels */}
          <div className="border-r border-border">
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

          {/* Day columns */}
          {days.map((day, dayIdx) => (
            <div key={dayIdx} className="relative border-r border-border">
              {/* Hour slots */}
              {hours.map((hour) => (
                <div
                  key={hour}
                  onClick={() => handleTimeSlotClick(day, hour)}
                  className="h-[60px] cursor-pointer border-b border-border transition-colors hover:bg-accent/30"
                />
              ))}

              {/* Current time indicator */}
              {isToday(day) && (
                <div
                  className="pointer-events-none absolute left-0 right-0 z-10 border-t-2 border-red-500"
                  style={{ top: `${currentMinutes}px` }}
                >
                  <div className="absolute -left-1 -top-1.5 size-3 rounded-full bg-red-500" />
                </div>
              )}

              {/* Timed events */}
              {(timedEventsByDay.get(dayIdx) ?? []).map((event) => {
                const pos = getEventPosition(
                  event.start_time,
                  event.end_time,
                  day,
                );
                const colors = getEventColorClasses(event.color);
                return (
                  <button
                    key={event.id}
                    type="button"
                    onClick={(e) => handleEventClick(e, event.id)}
                    className={cn(
                      "absolute left-0.5 right-0.5 overflow-hidden rounded px-1 py-0.5 text-left text-[10px] leading-tight",
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
                      {formatTime(event.start_time, timeFormat)}
                    </div>
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
