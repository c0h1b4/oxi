import { humanizeDate as humanize } from "@/lib/i18n/format";

/** Format bytes as human-readable file size. */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/** Convert ISO date string to human-readable relative or absolute format. */
export function humanizeDate(locale: string, iso: string): string {
  return humanize(locale, iso, { withYear: true });
}
