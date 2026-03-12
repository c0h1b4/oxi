/**
 * Calendar utility functions for date manipulation and formatting.
 */

/** Preset event colors. */
export const EVENT_COLORS = [
  { name: "Blue", value: "blue", bg: "bg-blue-500", text: "text-white" },
  { name: "Red", value: "red", bg: "bg-red-500", text: "text-white" },
  { name: "Green", value: "green", bg: "bg-green-500", text: "text-white" },
  { name: "Yellow", value: "yellow", bg: "bg-yellow-500", text: "text-white" },
  { name: "Purple", value: "purple", bg: "bg-purple-500", text: "text-white" },
  { name: "Pink", value: "pink", bg: "bg-pink-500", text: "text-white" },
  { name: "Orange", value: "orange", bg: "bg-orange-500", text: "text-white" },
  { name: "Teal", value: "teal", bg: "bg-teal-500", text: "text-white" },
] as const;

/** Reminder options in minutes. */
export const REMINDER_OPTIONS = [
  { label: "None", value: null },
  { label: "5 minutes", value: 5 },
  { label: "15 minutes", value: 15 },
  { label: "30 minutes", value: 30 },
  { label: "1 hour", value: 60 },
  { label: "1 day", value: 1440 },
] as const;

/** Get the color classes for an event by its color value. */
export function getEventColorClasses(color: string | null): {
  bg: string;
  text: string;
} {
  const found = EVENT_COLORS.find((c) => c.value === color);
  return found
    ? { bg: found.bg, text: found.text }
    : { bg: "bg-blue-500", text: "text-white" };
}

/** Get days in a month grid (including padding days from prev/next months). */
export function getMonthGrid(
  year: number,
  month: number,
  weekStartsOn: number,
): Date[] {
  const firstDay = new Date(year, month, 1);
  const lastDay = new Date(year, month + 1, 0);

  // Day of week for the first of the month (0=Sun, 6=Sat)
  let startOffset = firstDay.getDay() - weekStartsOn;
  if (startOffset < 0) startOffset += 7;

  const days: Date[] = [];

  // Previous month padding
  for (let i = startOffset - 1; i >= 0; i--) {
    const d = new Date(year, month, -i);
    days.push(d);
  }

  // Current month days
  for (let d = 1; d <= lastDay.getDate(); d++) {
    days.push(new Date(year, month, d));
  }

  // Next month padding to fill the grid (always show 6 rows = 42 cells)
  while (days.length < 42) {
    const nextDay = new Date(year, month + 1, days.length - startOffset - lastDay.getDate() + 1);
    days.push(nextDay);
  }

  return days;
}

/** Get the 7 days of a week that contains the given date. */
export function getWeekDays(date: Date, weekStartsOn: number): Date[] {
  const d = new Date(date);
  let dayOfWeek = d.getDay() - weekStartsOn;
  if (dayOfWeek < 0) dayOfWeek += 7;

  const start = new Date(d);
  start.setDate(d.getDate() - dayOfWeek);

  const days: Date[] = [];
  for (let i = 0; i < 7; i++) {
    const day = new Date(start);
    day.setDate(start.getDate() + i);
    days.push(day);
  }
  return days;
}

/** Get abbreviated day names starting from the given week start. */
export function getDayNames(weekStartsOn: number): string[] {
  const names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const result: string[] = [];
  for (let i = 0; i < 7; i++) {
    result.push(names[(i + weekStartsOn) % 7]);
  }
  return result;
}

/** Check if two dates are the same calendar day. */
export function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

/** Check if a date is today. */
export function isToday(date: Date): boolean {
  return isSameDay(date, new Date());
}

/** Format a date as YYYY-MM-DD. */
export function formatDateISO(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

/** Format a date as YYYY-MM-DDTHH:mm. */
export function formatDateTimeLocal(date: Date): string {
  const y = date.getFullYear();
  const mo = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  const h = String(date.getHours()).padStart(2, "0");
  const mi = String(date.getMinutes()).padStart(2, "0");
  return `${y}-${mo}-${d}T${h}:${mi}`;
}

/** Format time in 12h or 24h format from a datetime string. */
export function formatTime(
  dateStr: string,
  format: string = "12h",
): string {
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return dateStr;

  const hours = date.getHours();
  const minutes = String(date.getMinutes()).padStart(2, "0");

  if (format === "24h") {
    return `${String(hours).padStart(2, "0")}:${minutes}`;
  }

  const period = hours >= 12 ? "PM" : "AM";
  const h12 = hours % 12 || 12;
  return `${h12}:${minutes} ${period}`;
}

/** Format month and year for header display. */
export function formatMonthYear(date: Date): string {
  return date.toLocaleDateString("en-US", { month: "long", year: "numeric" });
}

/** Format a date for day view header. */
export function formatDayHeader(date: Date): string {
  return date.toLocaleDateString("en-US", {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: "numeric",
  });
}

/** Get the start of the visible range for month view. */
export function getMonthViewRange(
  year: number,
  month: number,
  weekStartsOn: number,
): { start: string; end: string } {
  const grid = getMonthGrid(year, month, weekStartsOn);
  const first = grid[0];
  const last = grid[grid.length - 1];
  return {
    start: `${formatDateISO(first)}T00:00:00`,
    end: `${formatDateISO(last)}T23:59:59`,
  };
}

/** Get the visible range for a week view. */
export function getWeekViewRange(
  date: Date,
  weekStartsOn: number,
): { start: string; end: string } {
  const days = getWeekDays(date, weekStartsOn);
  return {
    start: `${formatDateISO(days[0])}T00:00:00`,
    end: `${formatDateISO(days[6])}T23:59:59`,
  };
}

/** Get the visible range for a day view. */
export function getDayViewRange(date: Date): { start: string; end: string } {
  const iso = formatDateISO(date);
  return {
    start: `${iso}T00:00:00`,
    end: `${iso}T23:59:59`,
  };
}

/** Hours array 0-23 for time grid. */
export function getHours(): number[] {
  return Array.from({ length: 24 }, (_, i) => i);
}

/** Calculate top position and height for an event in a time grid. */
export function getEventPosition(
  startTime: string,
  endTime: string,
  dayStart: Date,
): { top: number; height: number } {
  const start = new Date(startTime);
  const end = new Date(endTime);

  const dayStartMs = new Date(dayStart).setHours(0, 0, 0, 0);
  const dayEndMs = dayStartMs + 24 * 60 * 60 * 1000;

  const effectiveStart = Math.max(start.getTime(), dayStartMs);
  const effectiveEnd = Math.min(end.getTime(), dayEndMs);

  const startMinutes = (effectiveStart - dayStartMs) / (1000 * 60);
  const durationMinutes = (effectiveEnd - effectiveStart) / (1000 * 60);

  // Each hour = 60px
  const pixelsPerMinute = 60 / 60;
  return {
    top: startMinutes * pixelsPerMinute,
    height: Math.max(durationMinutes * pixelsPerMinute, 20),
  };
}
