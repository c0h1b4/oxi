import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCalendarStore } from "@/stores/useCalendarStore";
import { useUiStore } from "@/stores/useUiStore";
import { EventDetail } from "../EventDetail";

const { useCalendarEvent } = vi.hoisted(() => ({ useCalendarEvent: vi.fn() }));
vi.mock("@/hooks/useCalendar", () => ({
  useCalendarEvent,
  useDeleteEvent: () => ({ mutate: vi.fn(), isPending: false }),
  useSendEventInvitations: () => ({ mutate: vi.fn(), isPending: false }),
}));

afterEach(() => {
  cleanup();
  useCalendarStore.setState({ selectedEvent: null });
  useUiStore.setState({ effectiveAnimationMode: "medium", locale: "en" });
});

describe("imported event date tolerance", () => {
  it.each([
    [true, "2026-06-15", "2026-99-99"],
    [true, "malformed", "2026-06-15"],
    [false, "malformed", "2026-06-15T12:00:00Z"],
  ])("opens details with invalid dates (all day: %s)", (all_day, start_time, end_time) => {
    useUiStore.setState({ effectiveAnimationMode: "off", locale: "es" });
    useCalendarStore.setState({ selectedEvent: "imported" });
    useCalendarEvent.mockReturnValue({ data: {
      id: "imported", title: "Imported event", all_day, start_time, end_time,
      color: "blue", attendees: "[]", source: "ics",
    } });
    render(<EventDetail timeFormat="24h" />);
    expect(screen.getByRole("heading", { name: "Imported event" })).toBeTruthy();
    expect(screen.getByText(/Invalid Date/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Close event details" })).toBeTruthy();
  });
});
