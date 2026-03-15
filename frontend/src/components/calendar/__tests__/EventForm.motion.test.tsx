import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

vi.mock("framer-motion", async () => {
  function AnimatePresence({ children }: { children: ReactNode }) {
    return <>{children}</>;
  }

  return {
    AnimatePresence,
    motion: {
      div: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
    },
  };
});

const { mockCalendarState, mockUiState } = vi.hoisted(() => ({
  mockCalendarState: {
    showEventForm: true,
    editingEventId: null as string | null,
    closeEventForm: vi.fn(),
    selectedDate: new Date("2026-03-14T09:00:00Z"),
  },
  mockUiState: {
    effectiveAnimationMode: "medium" as "rich" | "medium" | "subtle" | "off",
  },
}));

vi.mock("@/stores/useCalendarStore", () => ({
  useCalendarStore: (selector: (state: typeof mockCalendarState) => unknown) =>
    selector(mockCalendarState),
}));

vi.mock("@/stores/useUiStore", () => ({
  useUiStore: (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState),
}));

vi.mock("@/hooks/useCalendar", () => ({
  useCalendarEvent: () => ({ data: null }),
  useCreateEvent: () => ({ isPending: false, mutate: vi.fn() }),
  useUpdateEvent: () => ({ isPending: false, mutate: vi.fn() }),
}));

import { EventForm } from "../EventForm";

describe("EventForm motion transitions", () => {
  it("animates overlay/content in non-off modes", () => {
    mockUiState.effectiveAnimationMode = "medium";
    render(<EventForm />);

    expect(screen.getByTestId("calendar-event-form-overlay-transition")).toBeTruthy();
    expect(screen.getByTestId("calendar-event-form-content-transition")).toBeTruthy();
  });

  it("keeps static modal in off mode", () => {
    mockUiState.effectiveAnimationMode = "off";
    render(<EventForm />);

    expect(screen.queryByTestId("calendar-event-form-overlay-transition")).toBeNull();
    expect(screen.queryByTestId("calendar-event-form-content-transition")).toBeNull();
    expect(screen.getByText("New Event")).toBeTruthy();
  });
});
