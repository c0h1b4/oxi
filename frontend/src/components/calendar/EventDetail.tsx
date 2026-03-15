"use client";

import { useMemo, useEffect } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import {
  X,
  MapPin,
  Clock,
  Users,
  Video,
  Pencil,
  Trash2,
  ExternalLink,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { useCalendarStore } from "@/stores/useCalendarStore";
import { useUiStore } from "@/stores/useUiStore";
import { useCalendarEvent, useDeleteEvent } from "@/hooks/useCalendar";
import { formatTime, getEventColorClasses } from "./calendarUtils";
import { createFadeSlideVariants, createScaleFadeVariants } from "@/lib/motion/variants";
import { cn } from "@/lib/utils";

interface EventDetailProps {
  timeFormat: string;
}

export function EventDetail({ timeFormat }: EventDetailProps) {
  const selectedEvent = useCalendarStore((s) => s.selectedEvent);
  const selectEvent = useCalendarStore((s) => s.selectEvent);
  const openEventForm = useCalendarStore((s) => s.openEventForm);
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const shouldAnimate = effectiveAnimationMode !== "off";
  const overlayMotionProps = createFadeSlideVariants(effectiveAnimationMode, "y");
  const contentMotionProps = createScaleFadeVariants(effectiveAnimationMode);
  const ContentContainer = shouldAnimate ? motion.div : "div";

  const { data: event } = useCalendarEvent(selectedEvent);
  const deleteEvent = useDeleteEvent();

  // Close on Escape
  useEffect(() => {
    if (!selectedEvent) return;
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") selectEvent(null);
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [selectedEvent, selectEvent]);

  const attendees = useMemo(() => {
    if (!event) return [];
    try {
      const parsed = JSON.parse(event.attendees);
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  }, [event]);

  const handleEdit = () => {
    if (!event) return;
    selectEvent(null);
    openEventForm(event.id);
  };

  const handleDelete = () => {
    if (!event) return;
    deleteEvent.mutate(event.id, {
      onSuccess: () => selectEvent(null),
    });
  };

  if (!selectedEvent || !event) return null;
  if (typeof document === "undefined") return null;

  const colors = getEventColorClasses(event.color);

  const formatDateRange = () => {
    if (event.all_day) {
      const start = new Date(event.start_time);
      const end = new Date(event.end_time);
      const startStr = start.toLocaleDateString("en-US", {
        weekday: "short",
        month: "short",
        day: "numeric",
      });
      const endStr = end.toLocaleDateString("en-US", {
        weekday: "short",
        month: "short",
        day: "numeric",
      });
      return startStr === endStr ? startStr : `${startStr} - ${endStr}`;
    }

    const start = new Date(event.start_time);
    const dateStr = start.toLocaleDateString("en-US", {
      weekday: "short",
      month: "short",
      day: "numeric",
    });
    return `${dateStr}, ${formatTime(event.start_time, timeFormat)} - ${formatTime(event.end_time, timeFormat)}`;
  };

  return createPortal(
    <AnimatePresence>
      <div className="fixed inset-0 z-50 flex items-center justify-center">
      {shouldAnimate ? (
        <motion.div
          data-testid="calendar-event-detail-overlay-transition"
          data-motion-props={JSON.stringify(overlayMotionProps)}
          initial={overlayMotionProps.initial}
          animate={overlayMotionProps.animate}
          exit={overlayMotionProps.exit}
          className="absolute inset-0 bg-black/50"
          onClick={() => selectEvent(null)}
        />
      ) : (
        <div
          className="absolute inset-0 bg-black/50"
          onClick={() => selectEvent(null)}
        />
      )}

      <ContentContainer
        {...(shouldAnimate
          ? {
              "data-testid": "calendar-event-detail-content-transition",
              "data-motion-props": JSON.stringify(contentMotionProps),
              initial: contentMotionProps.initial,
              animate: contentMotionProps.animate,
              exit: contentMotionProps.exit,
            }
          : {})}
        className="relative z-10 w-full max-w-md rounded-lg border border-border bg-background shadow-xl"
      >
        {/* Color bar at top */}
        <div className={cn("h-2 rounded-t-lg", colors.bg)} />

        <div className="p-5">
          {/* Header */}
          <div className="flex items-start justify-between mb-3">
            <h2 className="text-lg font-semibold text-foreground pr-8">
              {event.title}
            </h2>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => selectEvent(null)}
              className="shrink-0 text-muted-foreground"
            >
              <X className="size-4" />
            </Button>
          </div>

          {/* Details */}
          <div className="space-y-3">
            {/* Time */}
            <div className="flex items-center gap-2 text-sm text-foreground">
              <Clock className="size-4 text-muted-foreground" />
              <span>{formatDateRange()}</span>
              {event.all_day && (
                <span className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                  All day
                </span>
              )}
            </div>

            {/* Location */}
            {event.location && (
              <div className="flex items-center gap-2 text-sm text-foreground">
                <MapPin className="size-4 text-muted-foreground" />
                <span>{event.location}</span>
              </div>
            )}

            {/* Meeting URL */}
            {event.meeting_url && (
              <div className="flex items-center gap-2 text-sm">
                <Video className="size-4 text-muted-foreground" />
                <a
                  href={event.meeting_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1 text-primary hover:underline"
                >
                  Join Meeting
                  <ExternalLink className="size-3" />
                </a>
              </div>
            )}

            {/* Attendees */}
            {attendees.length > 0 && (
              <div className="flex items-start gap-2 text-sm text-foreground">
                <Users className="mt-0.5 size-4 text-muted-foreground" />
                <div className="space-y-0.5">
                  {attendees.map((email: string) => (
                    <div key={email} className="text-sm">
                      {email}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Organizer */}
            {event.organizer && (
              <div className="text-xs text-muted-foreground">
                Organizer: {event.organizer}
              </div>
            )}

            {/* Description */}
            {event.description && (
              <div className="mt-3 rounded-md bg-muted/50 p-3 text-sm text-foreground whitespace-pre-wrap">
                {event.description}
              </div>
            )}

            {/* Source info */}
            {event.source !== "manual" && (
              <div className="text-xs text-muted-foreground">
                Source: {event.source}
              </div>
            )}
          </div>

          {/* Actions */}
          <div className="mt-4 flex justify-end gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={handleEdit}
              className="gap-1.5"
            >
              <Pencil className="size-3.5" />
              Edit
            </Button>
            <Button
              variant="destructive"
              size="sm"
              onClick={handleDelete}
              disabled={deleteEvent.isPending}
              className="gap-1.5"
            >
              <Trash2 className="size-3.5" />
              {deleteEvent.isPending ? "Deleting..." : "Delete"}
            </Button>
          </div>
        </div>
      </ContentContainer>
    </div>
    </AnimatePresence>,
    document.body,
  );
}
