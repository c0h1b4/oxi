import { describe, expect, it } from "vitest";
import { buildForwardBodyHtml, buildReplyBody, buildReplyBodyHtml } from "@/lib/email-utils";
import { formatDateTime } from "@/lib/i18n/format";

const date = "2026-10-26T12:00:00Z";

describe("localized quote dates", () => {
  it("uses the selected locale and preserves the year", () => {
    const body = buildReplyBody("es", "Original", "sender@example.com", date);
    expect(body).toContain(formatDateTime("es", date, { withYear: true }));
    expect(body).toContain("2026");
    expect(body).toContain("> Original");
  });

  it("does not interpolate malformed email dates into HTML", () => {
    const malformed = '<img src=x onerror="alert(1)">';
    expect(buildReplyBodyHtml("es", "<p>Original</p>", "sender@example.com", malformed)).not.toContain(malformed);
    expect(buildForwardBodyHtml("es", "<p>Original</p>", "sender@example.com", malformed, "Subject", "me@example.com")).not.toContain(malformed);
  });
});
