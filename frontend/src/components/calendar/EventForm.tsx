"use client";

import { useState, useCallback, useEffect } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useCalendarStore } from "@/stores/useCalendarStore";
import { useUiStore } from "@/stores/useUiStore";
import {
  useCreateEvent,
  useUpdateEvent,
  useCalendarEvent,
} from "@/hooks/useCalendar";
import {
  EVENT_COLORS,
  REMINDER_OPTIONS,
  formatDateTimeLocal,
} from "./calendarUtils";
import { createFadeSlideVariants, createScaleFadeVariants } from "@/lib/motion/variants";
import { cn } from "@/lib/utils";

export function EventForm() {
  const showEventForm = useCalendarStore((s) => s.showEventForm);
  const editingEventId = useCalendarStore((s) => s.editingEventId);
  const closeEventForm = useCalendarStore((s) => s.closeEventForm);
  const selectedDate = useCalendarStore((s) => s.selectedDate);
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const shouldAnimate = effectiveAnimationMode !== "off";
  const overlayMotionProps = createFadeSlideVariants(effectiveAnimationMode, "y");
  const contentMotionProps = createScaleFadeVariants(effectiveAnimationMode);
  const ContentContainer = shouldAnimate ? motion.div : "div";

  const { data: editingEvent } = useCalendarEvent(editingEventId);
  const createEvent = useCreateEvent();
  const updateEvent = useUpdateEvent();

  const isEditing = !!editingEventId;
  const isPending = createEvent.isPending || updateEvent.isPending;

  // Form state
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [location, setLocation] = useState("");
  const [startTime, setStartTime] = useState("");
  const [endTime, setEndTime] = useState("");
  const [allDay, setAllDay] = useState(false);
  const [meetingUrl, setMeetingUrl] = useState("");
  const [color, setColor] = useState("blue");
  const [reminderMinutes, setReminderMinutes] = useState<number | null>(null);
  const [attendees, setAttendees] = useState("");

  // Reset/populate form when dialog opens
  useEffect(() => {
    if (!showEventForm) return;

    if (editingEvent) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- intentional: reset form state when dialog opens with new event data
      setTitle(editingEvent.title);
      setDescription(editingEvent.description);
      setLocation(editingEvent.location);
      setStartTime(
        editingEvent.all_day
          ? editingEvent.start_time.slice(0, 10)
          : editingEvent.start_time.slice(0, 16),
      );
      setEndTime(
        editingEvent.all_day
          ? editingEvent.end_time.slice(0, 10)
          : editingEvent.end_time.slice(0, 16),
      );
      setAllDay(editingEvent.all_day);
      setMeetingUrl(editingEvent.meeting_url ?? "");
      setColor(editingEvent.color ?? "blue");
      setReminderMinutes(editingEvent.reminder_minutes);
      try {
        const parsed = JSON.parse(editingEvent.attendees);
        setAttendees(Array.isArray(parsed) ? parsed.join(", ") : "");
      } catch {
        setAttendees("");
      }
    } else {
      // Defaults for new event
      const start = new Date(selectedDate);
      if (start.getHours() === 0 && start.getMinutes() === 0) {
        start.setHours(9, 0, 0, 0);
      }
      const end = new Date(start);
      end.setHours(start.getHours() + 1);

      setTitle("");
      setDescription("");
      setLocation("");
      setStartTime(formatDateTimeLocal(start));
      setEndTime(formatDateTimeLocal(end));
      setAllDay(false);
      setMeetingUrl("");
      setColor("blue");
      setReminderMinutes(null);
      setAttendees("");
    }
  }, [showEventForm, editingEvent, selectedDate]);

  // Close on Escape
  useEffect(() => {
    if (!showEventForm) return;
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") closeEventForm();
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [showEventForm, closeEventForm]);

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!title.trim()) return;

      const attendeesList = attendees
        .split(",")
        .map((a) => a.trim())
        .filter((a) => a.length > 0);
      const attendeesJson = JSON.stringify(attendeesList);

      const formattedStart = allDay
        ? `${startTime}T00:00:00`
        : `${startTime}:00`;
      const formattedEnd = allDay
        ? `${endTime}T23:59:59`
        : `${endTime}:00`;

      if (isEditing && editingEventId) {
        updateEvent.mutate(
          {
            id: editingEventId,
            title: title.trim(),
            description: description.trim(),
            location: location.trim(),
            start_time: formattedStart,
            end_time: formattedEnd,
            all_day: allDay,
            meeting_url: meetingUrl.trim() || undefined,
            color,
            reminder_minutes: reminderMinutes ?? undefined,
            attendees: attendeesJson,
          },
          { onSuccess: () => closeEventForm() },
        );
      } else {
        createEvent.mutate(
          {
            title: title.trim(),
            description: description.trim(),
            location: location.trim(),
            start_time: formattedStart,
            end_time: formattedEnd,
            all_day: allDay,
            meeting_url: meetingUrl.trim() || undefined,
            color,
            reminder_minutes: reminderMinutes ?? undefined,
            attendees: attendeesJson,
          },
          { onSuccess: () => closeEventForm() },
        );
      }
    },
    [
      title,
      description,
      location,
      startTime,
      endTime,
      allDay,
      meetingUrl,
      color,
      reminderMinutes,
      attendees,
      isEditing,
      editingEventId,
      createEvent,
      updateEvent,
      closeEventForm,
    ],
  );

  if (!showEventForm) return null;
  if (typeof document === "undefined") return null;

  return createPortal(
    <AnimatePresence>
      <div className="fixed inset-0 z-50 flex items-center justify-center">
      {shouldAnimate ? (
        <motion.div
          data-testid="calendar-event-form-overlay-transition"
          data-motion-props={JSON.stringify(overlayMotionProps)}
          initial={overlayMotionProps.initial}
          animate={overlayMotionProps.animate}
          exit={overlayMotionProps.exit}
          className="absolute inset-0 bg-black/50"
          onClick={closeEventForm}
        />
      ) : (
        <div
          className="absolute inset-0 bg-black/50"
          onClick={closeEventForm}
        />
      )}

      <ContentContainer
        {...(shouldAnimate
          ? {
              "data-testid": "calendar-event-form-content-transition",
              "data-motion-props": JSON.stringify(contentMotionProps),
              initial: contentMotionProps.initial,
              animate: contentMotionProps.animate,
              exit: contentMotionProps.exit,
            }
          : {})}
        className="relative z-10 w-full max-w-lg max-h-[90vh] overflow-y-auto rounded-lg border border-border bg-background p-6 shadow-xl"
      >
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-foreground">
            {isEditing ? "Edit Event" : "New Event"}
          </h2>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={closeEventForm}
            className="text-muted-foreground"
          >
            <X className="size-4" />
          </Button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Title */}
          <div className="space-y-1.5">
            <Label htmlFor="event-title">
              Title <span className="text-destructive">*</span>
            </Label>
            <Input
              id="event-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Event title"
              required
              autoFocus
            />
          </div>

          {/* All day toggle */}
          <div className="flex items-center gap-2">
            <input
              id="event-allday"
              type="checkbox"
              checked={allDay}
              onChange={(e) => setAllDay(e.target.checked)}
              className="size-4 rounded border-input"
            />
            <Label htmlFor="event-allday" className="cursor-pointer">
              All day
            </Label>
          </div>

          {/* Date/time inputs */}
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label htmlFor="event-start">Start</Label>
              <Input
                id="event-start"
                type={allDay ? "date" : "datetime-local"}
                value={startTime}
                onChange={(e) => setStartTime(e.target.value)}
                required
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="event-end">End</Label>
              <Input
                id="event-end"
                type={allDay ? "date" : "datetime-local"}
                value={endTime}
                onChange={(e) => setEndTime(e.target.value)}
                required
              />
            </div>
          </div>

          {/* Location */}
          <div className="space-y-1.5">
            <Label htmlFor="event-location">Location</Label>
            <Input
              id="event-location"
              value={location}
              onChange={(e) => setLocation(e.target.value)}
              placeholder="Add location"
            />
          </div>

          {/* Description */}
          <div className="space-y-1.5">
            <Label htmlFor="event-description">Description</Label>
            <textarea
              id="event-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Add description..."
              rows={3}
              className="w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-xs placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] outline-none resize-none dark:bg-input/30"
            />
          </div>

          {/* Meeting URL */}
          <div className="space-y-1.5">
            <Label htmlFor="event-meeting-url">Meeting URL</Label>
            <Input
              id="event-meeting-url"
              type="url"
              value={meetingUrl}
              onChange={(e) => setMeetingUrl(e.target.value)}
              placeholder="https://zoom.us/j/..."
            />
          </div>

          {/* Attendees */}
          <div className="space-y-1.5">
            <Label htmlFor="event-attendees">
              Attendees (comma-separated emails)
            </Label>
            <Input
              id="event-attendees"
              value={attendees}
              onChange={(e) => setAttendees(e.target.value)}
              placeholder="alice@example.com, bob@example.com"
            />
          </div>

          {/* Color */}
          <div className="space-y-1.5">
            <Label>Color</Label>
            <div className="flex gap-1.5">
              {EVENT_COLORS.map((c) => (
                <button
                  key={c.value}
                  type="button"
                  onClick={() => setColor(c.value)}
                  className={cn(
                    "size-6 rounded-full transition-all",
                    c.bg,
                    color === c.value
                      ? "ring-2 ring-foreground ring-offset-2 ring-offset-background"
                      : "opacity-60 hover:opacity-100",
                  )}
                  title={c.name}
                />
              ))}
            </div>
          </div>

          {/* Reminder */}
          <div className="space-y-1.5">
            <Label htmlFor="event-reminder">Reminder</Label>
            <select
              id="event-reminder"
              value={reminderMinutes ?? ""}
              onChange={(e) =>
                setReminderMinutes(
                  e.target.value ? Number(e.target.value) : null,
                )
              }
              className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] outline-none dark:bg-input/30"
            >
              {REMINDER_OPTIONS.map((opt) => (
                <option key={opt.label} value={opt.value ?? ""}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>

          {/* Actions */}
          <div className="flex justify-end gap-2 pt-2">
            <Button
              type="button"
              variant="outline"
              onClick={closeEventForm}
              disabled={isPending}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={isPending || !title.trim()}>
              {isPending
                ? "Saving..."
                : isEditing
                  ? "Update Event"
                  : "Create Event"}
            </Button>
          </div>
        </form>
      </ContentContainer>
    </div>
    </AnimatePresence>,
    document.body,
  );
}
