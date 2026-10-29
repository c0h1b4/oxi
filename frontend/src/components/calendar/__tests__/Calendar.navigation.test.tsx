import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useCalendarStore } from "@/stores/useCalendarStore";
import { CalendarHeader } from "../CalendarHeader";
import { MonthView } from "../MonthView";

vi.mock("@/hooks/useCalendar", () => ({
  useCalendarEvents: () => ({ data: { events: [] } }),
}));

describe("Calendar touch navigation", () => {
  beforeEach(() => {
    useCalendarStore.setState({
      selectedDate: new Date(2026, 9, 15),
      viewMode: "month",
      showEventForm: false,
    });
  });

  it("opens a day with one click instead of requiring a double click", () => {
    render(<MonthView weekStartsOn={0} timeFormat="12h" />);
    fireEvent.click(screen.getByRole("button", { name: "Thursday, October 15, 2026" }));
    expect(useCalendarStore.getState().viewMode).toBe("day");
    expect(useCalendarStore.getState().selectedDate.getDate()).toBe(15);
  });

  it("keeps period navigation, view selection and event creation available", () => {
    render(<CalendarHeader />);
    fireEvent.click(screen.getByRole("button", { name: "Day" }));
    expect(screen.getByRole("button", { name: "Day" }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Next period" }));
    expect(useCalendarStore.getState().selectedDate.getDate()).toBe(16);
    fireEvent.click(screen.getByRole("button", { name: "Previous period" }));
    expect(useCalendarStore.getState().selectedDate.getDate()).toBe(15);
    fireEvent.click(screen.getByRole("button", { name: "Event" }));
    expect(useCalendarStore.getState().showEventForm).toBe(true);
  });
});
