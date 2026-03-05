export interface IcsEvent {
  summary: string;
  dtstart: Date | null;
  dtend: Date | null;
  isAllDay: boolean;
  location: string;
  description: string;
  organizer: string;
  attendees: string[];
}

function unfold(raw: string): string {
  return raw.replace(/\r?\n[ \t]/g, "");
}

function unescape(val: string): string {
  return val
    .replace(/\\n/gi, "\n")
    .replace(/\\,/g, ",")
    .replace(/\\;/g, ";")
    .replace(/\\\\/g, "\\");
}

function parseDateTime(value: string): { date: Date | null; allDay: boolean } {
  // All-day: YYYYMMDD
  if (/^\d{8}$/.test(value)) {
    const y = +value.slice(0, 4);
    const m = +value.slice(4, 6) - 1;
    const d = +value.slice(6, 8);
    return { date: new Date(y, m, d), allDay: true };
  }
  // UTC: YYYYMMDDTHHMMSSZ
  if (/^\d{8}T\d{6}Z$/i.test(value)) {
    const y = +value.slice(0, 4);
    const m = +value.slice(4, 6) - 1;
    const d = +value.slice(6, 8);
    const hh = +value.slice(9, 11);
    const mm = +value.slice(11, 13);
    const ss = +value.slice(13, 15);
    return { date: new Date(Date.UTC(y, m, d, hh, mm, ss)), allDay: false };
  }
  // Local: YYYYMMDDTHHMMSS
  if (/^\d{8}T\d{6}$/i.test(value)) {
    const y = +value.slice(0, 4);
    const m = +value.slice(4, 6) - 1;
    const d = +value.slice(6, 8);
    const hh = +value.slice(9, 11);
    const mm = +value.slice(11, 13);
    const ss = +value.slice(13, 15);
    return { date: new Date(y, m, d, hh, mm, ss), allDay: false };
  }
  return { date: null, allDay: false };
}

function extractCN(line: string): string {
  const cnMatch = line.match(/CN=([^;:]+)/i);
  if (cnMatch) return cnMatch[1].replace(/^"|"$/g, "");
  const mailtoMatch = line.match(/mailto:([^\s;]+)/i);
  if (mailtoMatch) return mailtoMatch[1];
  return "";
}

function getPropertyValue(line: string): string {
  // Value is everything after the first unescaped colon (skip params)
  const idx = line.indexOf(":");
  return idx >= 0 ? line.slice(idx + 1) : "";
}

export function parseIcs(raw: string): IcsEvent[] {
  const unfolded = unfold(raw);
  const lines = unfolded.split(/\r?\n/);
  const events: IcsEvent[] = [];
  let inEvent = false;
  let event: IcsEvent | null = null;

  for (const line of lines) {
    const upper = line.toUpperCase();

    if (upper === "BEGIN:VEVENT") {
      inEvent = true;
      event = {
        summary: "",
        dtstart: null,
        dtend: null,
        isAllDay: false,
        location: "",
        description: "",
        organizer: "",
        attendees: [],
      };
      continue;
    }

    if (upper === "END:VEVENT") {
      if (event) events.push(event);
      inEvent = false;
      event = null;
      continue;
    }

    if (!inEvent || !event) continue;

    if (upper.startsWith("SUMMARY")) {
      event.summary = unescape(getPropertyValue(line));
    } else if (upper.startsWith("DTSTART")) {
      const val = getPropertyValue(line);
      const parsed = parseDateTime(val);
      event.dtstart = parsed.date;
      if (parsed.allDay) event.isAllDay = true;
    } else if (upper.startsWith("DTEND")) {
      const val = getPropertyValue(line);
      const parsed = parseDateTime(val);
      event.dtend = parsed.date;
    } else if (upper.startsWith("LOCATION")) {
      event.location = unescape(getPropertyValue(line));
    } else if (upper.startsWith("DESCRIPTION")) {
      event.description = unescape(getPropertyValue(line));
    } else if (upper.startsWith("ORGANIZER")) {
      event.organizer = extractCN(line);
    } else if (upper.startsWith("ATTENDEE")) {
      const name = extractCN(line);
      if (name) event.attendees.push(name);
    }
  }

  return events;
}
