"use client";

import { ChevronLeft, ChevronRight, Plus, Calendar } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useCalendarStore } from "@/stores/useCalendarStore";
import { formatMonthYear, formatDayHeader } from "./calendarUtils";
import { cn } from "@/lib/utils";

export function CalendarHeader() {
  const selectedDate = useCalendarStore((s) => s.selectedDate);
  const viewMode = useCalendarStore((s) => s.viewMode);
  const setDate = useCalendarStore((s) => s.setDate);
  const setViewMode = useCalendarStore((s) => s.setViewMode);
  const openEventForm = useCalendarStore((s) => s.openEventForm);

  const goToday = () => setDate(new Date());

  const goPrev = () => {
    const d = new Date(selectedDate);
    if (viewMode === "month") {
      d.setMonth(d.getMonth() - 1);
    } else if (viewMode === "week") {
      d.setDate(d.getDate() - 7);
    } else {
      d.setDate(d.getDate() - 1);
    }
    setDate(d);
  };

  const goNext = () => {
    const d = new Date(selectedDate);
    if (viewMode === "month") {
      d.setMonth(d.getMonth() + 1);
    } else if (viewMode === "week") {
      d.setDate(d.getDate() + 7);
    } else {
      d.setDate(d.getDate() + 1);
    }
    setDate(d);
  };

  const headerText =
    viewMode === "day"
      ? formatDayHeader(selectedDate)
      : formatMonthYear(selectedDate);

  const viewModes: Array<{ label: string; value: "month" | "week" | "day" }> = [
    { label: "Month", value: "month" },
    { label: "Week", value: "week" },
    { label: "Day", value: "day" },
  ];

  return (
    <div className="flex items-center justify-between border-b border-border px-4 py-3">
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-2">
          <Calendar className="size-5 text-primary" />
          <h1 className="text-base font-semibold text-foreground">Calendar</h1>
        </div>

        <div className="flex items-center gap-1">
          <Button variant="outline" size="sm" onClick={goToday}>
            Today
          </Button>
          <Button variant="ghost" size="icon-sm" onClick={goPrev}>
            <ChevronLeft className="size-4" />
          </Button>
          <Button variant="ghost" size="icon-sm" onClick={goNext}>
            <ChevronRight className="size-4" />
          </Button>
        </div>

        <span className="text-sm font-medium text-foreground">{headerText}</span>
      </div>

      <div className="flex items-center gap-2">
        {/* View mode toggle */}
        <div className="flex rounded-md border border-input">
          {viewModes.map((m) => (
            <button
              key={m.value}
              type="button"
              onClick={() => setViewMode(m.value)}
              className={cn(
                "px-3 py-1 text-xs font-medium transition-colors first:rounded-l-md last:rounded-r-md",
                viewMode === m.value
                  ? "bg-primary text-primary-foreground"
                  : "bg-background text-muted-foreground hover:bg-accent",
              )}
            >
              {m.label}
            </button>
          ))}
        </div>

        <Button size="sm" onClick={() => openEventForm()} className="gap-1.5">
          <Plus className="size-4" />
          Event
        </Button>
      </div>
    </div>
  );
}
