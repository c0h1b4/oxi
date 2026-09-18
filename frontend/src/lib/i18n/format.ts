/**
 * Date formatting bound to the active locale.
 *
 * Every call site that used to hardcode "en-US" (or pass no locale at all)
 * routes through here, so switching language in Settings changes dates too.
 */

const dateTimeFormats = new Map<string, Intl.DateTimeFormat>();
const relativeTimeFormats = new Map<string, Intl.RelativeTimeFormat>();

/** Intl constructors are expensive; message lists format a date per row. */
function dateTimeFormat(locale: string, options: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
  const key = `${locale}:${JSON.stringify(options)}`;
  let format = dateTimeFormats.get(key);
  if (!format) {
    format = new Intl.DateTimeFormat(locale, options);
    dateTimeFormats.set(key, format);
  }
  return format;
}

function relativeTimeFormat(locale: string): Intl.RelativeTimeFormat {
  let format = relativeTimeFormats.get(locale);
  if (!format) {
    format = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
    relativeTimeFormats.set(locale, format);
  }
  return format;
}

export function formatDate(
  locale: string,
  date: Date,
  options: Intl.DateTimeFormatOptions,
): string {
  return dateTimeFormat(locale, options).format(date);
}

/**
 * Relative time for the recent past ("2 minutes ago", "yesterday").
 * Intl supplies both the wording and the plural form, so no English is baked in.
 */
export function formatRelativeTime(
  locale: string,
  value: number,
  unit: Intl.RelativeTimeFormatUnit,
): string {
  return relativeTimeFormat(locale).format(value, unit);
}

const HUMANIZED_DATE: Intl.DateTimeFormatOptions = {
  month: "short",
  day: "numeric",
  hour: "numeric",
  minute: "2-digit",
};

/**
 * An ISO timestamp as relative time within the last day, and as a short date
 * before that. Invalid input is returned untouched, as it was before.
 */
export function humanizeDate(
  locale: string,
  iso: string,
  options: { withYear?: boolean } = {},
): string {
  const date = new Date(iso);
  if (isNaN(date.getTime())) return iso;

  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMinutes = Math.floor(diffMs / 60_000);
  const diffHours = Math.floor(diffMs / 3_600_000);

  if (diffMinutes < 1) return formatRelativeTime(locale, 0, "second");
  if (diffMinutes < 60) return formatRelativeTime(locale, -diffMinutes, "minute");
  if (diffHours < 24) return formatRelativeTime(locale, -diffHours, "hour");

  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (
    date.getFullYear() === yesterday.getFullYear() &&
    date.getMonth() === yesterday.getMonth() &&
    date.getDate() === yesterday.getDate()
  ) {
    return formatRelativeTime(locale, -1, "day");
  }

  return formatDate(
    locale,
    date,
    options.withYear ? { ...HUMANIZED_DATE, year: "numeric" } : HUMANIZED_DATE,
  );
}

const LIST_TIME: Intl.DateTimeFormatOptions = { hour: "numeric", minute: "2-digit" };
const LIST_WEEKDAY: Intl.DateTimeFormatOptions = { weekday: "short" };
const LIST_DAY_MONTH: Intl.DateTimeFormatOptions = { month: "short", day: "numeric" };
const LIST_NUMERIC: Intl.DateTimeFormatOptions = {
  month: "2-digit",
  day: "2-digit",
  year: "2-digit",
};

/**
 * The compact date column of a message list: time today, weekday within the
 * week, day and month within the year, and a numeric date before that.
 */
export function formatListDate(locale: string, dateStr: string): string {
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return dateStr;

  const now = new Date();
  const isToday =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();

  const time = formatDate(locale, date, LIST_TIME);
  if (isToday) return time;

  const daysDiff = Math.floor((now.getTime() - date.getTime()) / 86_400_000);
  if (daysDiff < 7 && daysDiff >= 0) {
    return `${formatDate(locale, date, LIST_WEEKDAY)} ${time}`;
  }
  if (date.getFullYear() === now.getFullYear()) {
    return `${formatDate(locale, date, LIST_DAY_MONTH)} ${time}`;
  }
  return `${formatDate(locale, date, LIST_NUMERIC)} ${time}`;
}

const SHORT_DATE: Intl.DateTimeFormatOptions = {
  year: "numeric",
  month: "short",
  day: "numeric",
};

/** A plain calendar date. Invalid input is returned untouched. */
export function formatShortDate(locale: string, dateStr: string): string {
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return dateStr;
  return formatDate(locale, date, SHORT_DATE);
}

const DATE_TIME: Intl.DateTimeFormatOptions = {
  month: "short",
  day: "numeric",
  hour: "numeric",
  minute: "2-digit",
};

/** A date with the time of day, used in thread headers and quoted replies. */
export function formatDateTime(
  locale: string,
  dateStr: string,
  options: { withYear?: boolean } = {},
): string {
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return dateStr;
  return formatDate(
    locale,
    date,
    options.withYear ? { ...DATE_TIME, year: "numeric" } : DATE_TIME,
  );
}
