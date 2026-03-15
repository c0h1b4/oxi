"use client";

import { useEffect } from "react";
import { AnimatePresence, motion } from "framer-motion";
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
  const shouldAnimate = effectiveAnimationMode !== "off";
  const panelTransition = createFadeSlideVariants(effectiveAnimationMode, "x");
  const PanelContainer = shouldAnimate ? motion.div : "div";
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
    <PanelContainer
      {...(shouldAnimate
        ? {
            "data-testid": "calendar-panel-transition",
            "data-motion-props": JSON.stringify(panelTransition),
            initial: panelTransition.initial,
            animate: panelTransition.animate,
            exit: panelTransition.exit,
          }
        : {})}
      className="flex h-full min-w-0 flex-1 flex-col"
    >
      <CalendarHeader />

      <div className="flex flex-1 overflow-hidden">
        {shouldAnimate ? (
          <AnimatePresence mode="wait" initial={false}>
            {viewMode === "month" && (
              <motion.div
                key="calendar-month-view"
                data-testid="calendar-month-view-transition"
                data-motion-props={JSON.stringify(panelTransition)}
                initial={panelTransition.initial}
                animate={panelTransition.animate}
                exit={panelTransition.exit}
                className="flex min-h-0 min-w-0 flex-1"
              >
                <MonthView weekStartsOn={weekStartsOn} timeFormat={timeFormat} />
              </motion.div>
            )}
            {viewMode === "week" && (
              <motion.div
                key="calendar-week-view"
                data-testid="calendar-week-view-transition"
                data-motion-props={JSON.stringify(panelTransition)}
                initial={panelTransition.initial}
                animate={panelTransition.animate}
                exit={panelTransition.exit}
                className="flex min-h-0 min-w-0 flex-1"
              >
                <WeekView weekStartsOn={weekStartsOn} timeFormat={timeFormat} />
              </motion.div>
            )}
            {viewMode === "day" && (
              <motion.div
                key="calendar-day-view"
                data-testid="calendar-day-view-transition"
                data-motion-props={JSON.stringify(panelTransition)}
                initial={panelTransition.initial}
                animate={panelTransition.animate}
                exit={panelTransition.exit}
                className="flex min-h-0 min-w-0 flex-1"
              >
                <DayView timeFormat={timeFormat} />
              </motion.div>
            )}
          </AnimatePresence>
        ) : (
          <>
            {viewMode === "month" && (
              <MonthView weekStartsOn={weekStartsOn} timeFormat={timeFormat} />
            )}
            {viewMode === "week" && (
              <WeekView weekStartsOn={weekStartsOn} timeFormat={timeFormat} />
            )}
            {viewMode === "day" && <DayView timeFormat={timeFormat} />}
          </>
        )}
      </div>

      <EventForm />
      <EventDetail timeFormat={timeFormat} />
    </PanelContainer>
  );
}
