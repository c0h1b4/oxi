import { apiUrl } from "@/lib/api-url";

/** Replace this draft's backend image URLs, including account queries, with MIME CIDs. */
export function inlineImagesToCid(html: string, draftId: string | null): string {
  if (!draftId) return html;
  const prefix = apiUrl(`/drafts/${draftId}/attachments/`);
  return html.replace(/\bsrc=(['"])(.*?)\1/gi, (attribute, quote, url: string) => {
    if (!url.startsWith(prefix)) return attribute;
    const match = /^([^/?]+)\/content(?:\?[^"']*)?$/.exec(url.slice(prefix.length));
    return match ? `src=${quote}cid:${match[1]}${quote}` : attribute;
  });
}

/** Count the total number of recipients across To, Cc, Bcc fields. */
export function countRecipients(...fields: string[]): number {
  return fields.reduce(
    (count, field) =>
      count +
      field
        .split(",")
        .map((s) => s.trim())
        .filter((s) => s.length > 0).length,
    0,
  );
}

/** Format bytes as human-readable file size. */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Strip HTML tags to produce a plain-text fallback. */
export function stripHtml(html: string): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  return doc.body.textContent ?? "";
}

/** Generate a UUID v4 (crypto-based). */
export function generateId(): string {
  return crypto.randomUUID();
}
