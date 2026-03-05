"use client";

import { useState, useEffect } from "react";
import {
  CalendarDays,
  Clock,
  MapPin,
  AlignLeft,
  User,
  Users,
  Download,
  Loader2,
} from "lucide-react";
import { parseIcs, type IcsEvent } from "@/lib/ics-parser";

interface IcsPreviewProps {
  url: string;
  filename: string | null;
}

function formatDate(date: Date): string {
  return date.toLocaleDateString(undefined, {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

function formatTime(date: Date): string {
  return date.toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

function EventCard({ event }: { event: IcsEvent }) {
  const dateTimeLabel = (() => {
    if (!event.dtstart) return null;
    if (event.isAllDay) {
      const line = formatDate(event.dtstart);
      return `${line} (all day)`;
    }
    const startDate = formatDate(event.dtstart);
    const startTime = formatTime(event.dtstart);
    if (event.dtend && isSameDay(event.dtstart, event.dtend)) {
      return `${startDate}, ${startTime} – ${formatTime(event.dtend)}`;
    }
    if (event.dtend) {
      return `${startDate} ${startTime} – ${formatDate(event.dtend)} ${formatTime(event.dtend)}`;
    }
    return `${startDate}, ${startTime}`;
  })();

  return (
    <div className="w-full max-w-lg overflow-hidden break-words rounded-lg border border-border bg-card p-5 text-left shadow-sm">
      {event.summary && (
        <div className="mb-4 flex items-start gap-3">
          <CalendarDays className="mt-0.5 size-5 shrink-0 text-primary" />
          <h3 className="text-base font-semibold text-foreground">
            {event.summary}
          </h3>
        </div>
      )}

      {dateTimeLabel && (
        <div className="mb-3 flex items-start gap-3">
          <Clock className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
          <p className="min-w-0 text-sm text-foreground">{dateTimeLabel}</p>
        </div>
      )}

      {event.location && (
        <div className="mb-3 flex items-start gap-3">
          <MapPin className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
          <p className="min-w-0 text-sm text-foreground">{event.location}</p>
        </div>
      )}

      {event.description && (
        <div className="mb-3 flex items-start gap-3">
          <AlignLeft className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
          <p className="min-w-0 whitespace-pre-wrap text-sm text-muted-foreground">
            {event.description}
          </p>
        </div>
      )}

      {event.organizer && (
        <div className="mb-3 flex items-start gap-3">
          <User className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
          <p className="text-sm text-foreground">{event.organizer}</p>
        </div>
      )}

      {event.attendees.length > 0 && (
        <div className="flex items-start gap-3">
          <Users className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
          <p className="text-sm text-foreground">
            {event.attendees.join(", ")}
          </p>
        </div>
      )}
    </div>
  );
}

export function IcsPreview({ url, filename }: IcsPreviewProps) {
  const [events, setEvents] = useState<IcsEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const res = await fetch(url, { credentials: "same-origin" });
        if (!res.ok) throw new Error(`Failed to fetch (${res.status})`);
        const text = await res.text();
        const parsed = parseIcs(text);
        if (!cancelled) {
          setEvents(parsed);
          setLoading(false);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Failed to load");
          setLoading(false);
        }
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, [url]);

  if (loading) {
    return (
      <div className="flex flex-col items-center gap-3 text-muted-foreground">
        <Loader2 className="size-8 animate-spin" />
        <p className="text-sm">Loading calendar event…</p>
      </div>
    );
  }

  if (error || events.length === 0) {
    return (
      <div className="flex flex-col items-center gap-4 text-center">
        <CalendarDays className="size-12 text-muted-foreground" />
        <p className="text-sm text-muted-foreground">
          {error ?? "No events found in this file"}
        </p>
        <a
          href={url}
          download={filename ?? undefined}
          className="inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
        >
          <Download className="size-4" />
          Download
        </a>
      </div>
    );
  }

  return (
    <div className="flex flex-col items-center gap-4">
      {events.map((event, i) => (
        <EventCard key={i} event={event} />
      ))}
    </div>
  );
}
