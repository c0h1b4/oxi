import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useCalendarStore } from "@/stores/useCalendarStore";
import { useUiStore } from "@/stores/useUiStore";
import { EventForm } from "../EventForm";
import { EventDetail } from "../EventDetail";

const mocks = vi.hoisted(() => ({
  event: { current: null as Record<string, unknown> | null },
  create: vi.fn(), update: vi.fn(), send: vi.fn(), warning: vi.fn(), success: vi.fn(),
}));
vi.mock("@/hooks/useCalendar", () => ({
  useCalendarEvent: () => ({ data: mocks.event.current }),
  useCreateEvent: () => ({ mutate: mocks.create, isPending: false }),
  useUpdateEvent: () => ({ mutate: mocks.update, isPending: false }),
  useDeleteEvent: () => ({ mutate: vi.fn(), isPending: false }),
  useSendEventInvitations: () => ({ mutate: mocks.send, isPending: false }),
}));
vi.mock("sonner", () => ({ toast: { warning: mocks.warning, success: mocks.success } }));
vi.mock("@/hooks/useAutocomplete", () => ({
  useAutocomplete: () => ({ results: [
    { item: { email: "alice@example.com", name: "Alice", source: "contact" }, highlightedEmail: "alice@example.com" },
    { item: { email: "alex@example.com", name: "Alex", source: "known" }, highlightedEmail: "alex@example.com" },
  ], isLoading: false }),
}));

beforeEach(() => {
  vi.clearAllMocks();
  mocks.event.current = null;
  useUiStore.setState({ effectiveAnimationMode: "off" });
  useCalendarStore.setState({ showEventForm: true, editingEventId: null, selectedEvent: null });
});
afterEach(cleanup);

function editEvent(source = "manual") {
  mocks.event.current = {
    id: "meeting", title: "Planning", description: "", location: "", organizer: "", source,
    start_time: "2026-11-15T09:00:00", end_time: "2026-11-15T10:00:00",
    all_day: false, attendees: '["alice@example.com"]', reminder_minutes: null, color: "blue",
  };
  useCalendarStore.setState({ editingEventId: "meeting" });
}

describe("calendar invitations", () => {
  it("offers contacts and known addresses and saves selected emails without display names", () => {
    render(<EventForm />);
    const attendees = screen.getByLabelText("Attendees (comma-separated emails)");
    fireEvent.change(attendees, { target: { value: "bob@example.com, al" } });
    expect(screen.getByText("Alice")).toBeTruthy();
    expect(screen.getByText("From email history")).toBeTruthy();
    fireEvent.keyDown(attendees, { key: "Enter" });
    expect((attendees as HTMLTextAreaElement).value).toBe("bob@example.com, alice@example.com, ");
    fireEvent.change(screen.getByLabelText(/Title/), { target: { value: "Planning" } });
    fireEvent.click(screen.getByRole("button", { name: "Create Event" }));
    expect(mocks.create).toHaveBeenCalledWith(expect.objectContaining({
      attendees: '["bob@example.com","alice@example.com"]',
    }), expect.anything());
  });

  it.each([["Save and send", true], ["Save without sending", false]])(
    "asks before saving an edit and supports %s", async (button, send) => {
      editEvent();
      render(<EventForm />);
      fireEvent.click(screen.getByRole("button", { name: "Update Event" }));
      expect(mocks.update).not.toHaveBeenCalled();
      expect(screen.getByRole("alertdialog")).toBeTruthy();
      fireEvent.click(screen.getByRole("button", { name: button }));
      await waitFor(() => expect(mocks.update).toHaveBeenCalledWith(expect.objectContaining({
        id: "meeting", send_invitations: send,
      }), expect.anything()));
    },
  );

  it("dismisses attendee suggestions with Escape without closing the form", () => {
    render(<EventForm />);
    const attendees = screen.getByLabelText("Attendees (comma-separated emails)");
    fireEvent.change(attendees, { target: { value: "al" } });
    fireEvent.keyDown(attendees, { key: "Escape" });
    expect(screen.queryByText("Alice")).toBeNull();
    expect(useCalendarStore.getState().showEventForm).toBe(true);
  });

  it("saves edits without a send prompt when there are no attendees", () => {
    editEvent();
    mocks.event.current!.attendees = "[]";
    render(<EventForm />);
    fireEvent.click(screen.getByRole("button", { name: "Update Event" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(mocks.update).toHaveBeenCalledWith(expect.objectContaining({ send_invitations: false }), expect.anything());
  });

  it("can cancel the update prompt without saving", () => {
    editEvent();
    render(<EventForm />);
    fireEvent.click(screen.getByRole("button", { name: "Update Event" }));
    fireEvent.click(screen.getByRole("alertdialog").querySelector("button")!);
    expect(mocks.update).not.toHaveBeenCalled();
  });

  it("does not offer sending updates for imported events", () => {
    editEvent("ics");
    render(<EventForm />);
    fireEvent.click(screen.getByRole("button", { name: "Update Event" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(mocks.update).toHaveBeenCalledWith(expect.objectContaining({ send_invitations: false }), expect.anything());
  });

  it("reports a saved event's invitation failure without encouraging duplicate creation", () => {
    mocks.create.mockImplementation((_body, options) => options.onSuccess({ invitation_error: "SMTP unavailable" }));
    render(<EventForm />);
    fireEvent.change(screen.getByLabelText(/Title/), { target: { value: "Planning" } });
    fireEvent.click(screen.getByRole("button", { name: "Create Event" }));
    expect(mocks.warning).toHaveBeenCalledWith(expect.stringContaining("Event saved, but invitations could not be sent"));
    expect(useCalendarStore.getState().showEventForm).toBe(false);
  });

  it("resends invitations from the saved event", () => {
    editEvent();
    useCalendarStore.setState({ showEventForm: false, selectedEvent: "meeting" });
    render(<EventDetail timeFormat="24h" />);
    fireEvent.click(screen.getByRole("button", { name: "Resend invitation" }));
    expect(mocks.send).toHaveBeenCalledWith("meeting", expect.anything());
  });

  it("does not offer resending for an external organizer", () => {
    editEvent();
    mocks.event.current!.organizer = "other@example.com";
    useCalendarStore.setState({ showEventForm: false, selectedEvent: "meeting" });
    render(<EventDetail timeFormat="24h" />);
    expect(screen.queryByRole("button", { name: "Resend invitation" })).toBeNull();
  });
});
