import { describe, it, expect } from "vitest";
import { findJunkFolder } from "../mail-folders";
import { isPdfAttachment } from "../mail-file-types";
import type { Folder } from "@/types/folder";
const folder = (name: string, attributes: string[] = [], delimiter: string | null = "/"): Folder => ({ name, attributes, delimiter, is_subscribed: true, total_count: 0, unread_count: 0 });
describe("Junk destination", () => {
  it("uses the real Spam folder instead of inventing Junk", () => expect(findJunkFolder([folder("INBOX"), folder("Spam")])).toBe("Spam"));
  it("prefers special-use flags over names", () => expect(findJunkFolder([folder("Junk"), folder("Unwanted", ["\\Junk"])])).toBe("Unwanted"));
  it("preserves nested server names", () => expect(findJunkFolder([folder("INBOX.Spam", [], ".")])).toBe("INBOX.Spam"));
  it("does not invent a missing destination", () => expect(findJunkFolder([folder("INBOX")])).toBeUndefined());
});
describe("PDF preview selection", () => {
  it("recognizes generic MIME metadata by filename", () => expect(isPdfAttachment("application/octet-stream", "Document.PDF")).toBe(true));
  it("recognizes parameterized MIME types", () => expect(isPdfAttachment("Application/PDF; name=a.pdf")).toBe(true));
  it("does not classify arbitrary files as PDFs", () => expect(isPdfAttachment("application/zip", "a.zip")).toBe(false));
});
