"use client";

import { useEffect, useMemo } from "react";
import { AnimatePresence } from "framer-motion";
import { AnimatedDiv } from "@/lib/motion/AnimatedDiv";
import { useCalendarStore } from "@/stores/useCalendarStore";
import { useUiStore } from "@/stores/useUiStore";
import { useCalendarSettings } from "@/hooks/useCalendar";
import { CalendarHeader } from "./CalendarHeader";
import { MonthView } from "./MonthView";
import { WeekView } from "./WeekView";
import { DayView } from "./DayView";
import { EventForm } from "./EventForm";
import { EventDetail } from "./EventDetail";
import { createFadeSlideVariants } from "@/lib/motion/variants";

export function CalendarPanel() {
  const viewMode = useCalendarStore((s) => s.viewMode);
  const setViewMode = useCalendarStore((s) => s.setViewMode);
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const panelTransition = useMemo(() => createFadeSlideVariants(effectiveAnimationMode, "x"), [effectiveAnimationMode]);
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
    <AnimatedDiv
      data-testid="calendar-panel-transition"
      variants={panelTransition}
      initial={panelTransition.initial}
      animate={panelTransition.animate}
      exit={panelTransition.exit}
      className="flex h-full min-w-0 flex-1 flex-col"
    >
      <CalendarHeader />

      <div className="flex flex-1 overflow-hidden">
        <AnimatePresence mode="wait" initial={false}>
          {viewMode === "month" && (
            <AnimatedDiv
              key="calendar-month-view"
              data-testid="calendar-month-view-transition"
              variants={panelTransition}
              initial={panelTransition.initial}
              animate={panelTransition.animate}
              exit={panelTransition.exit}
              className="flex min-h-0 min-w-0 flex-1"
            >
              <MonthView weekStartsOn={weekStartsOn} timeFormat={timeFormat} />
            </AnimatedDiv>
          )}
          {viewMode === "week" && (
            <AnimatedDiv
              key="calendar-week-view"
              data-testid="calendar-week-view-transition"
              variants={panelTransition}
              initial={panelTransition.initial}
              animate={panelTransition.animate}
              exit={panelTransition.exit}
              className="flex min-h-0 min-w-0 flex-1"
            >
              <WeekView weekStartsOn={weekStartsOn} timeFormat={timeFormat} />
            </AnimatedDiv>
          )}
          {viewMode === "day" && (
            <AnimatedDiv
              key="calendar-day-view"
              data-testid="calendar-day-view-transition"
              variants={panelTransition}
              initial={panelTransition.initial}
              animate={panelTransition.animate}
              exit={panelTransition.exit}
              className="flex min-h-0 min-w-0 flex-1"
            >
              <DayView timeFormat={timeFormat} />
            </AnimatedDiv>
          )}
        </AnimatePresence>
      </div>

      <EventForm />
      <EventDetail timeFormat={timeFormat} />
    </AnimatedDiv>
  );
}
