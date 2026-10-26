import { describe, expect, it } from "vitest";
import { Editor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { SignatureBlock } from "../SignatureBlock";
import { convertBodyFormat, injectSignature, removeSignatureFromBody, replaceSignatureInBody } from "@/stores/useComposeStore";

describe("signature editor round-trip", () => {
  it("retains a removable signature through text conversion and editor normalization", () => {
    const signature = "<div><p>Me</p><p>Company</p></div>";
    const original = injectSignature("<p>Reply &amp; details</p><blockquote><p>Original</p></blockquote>", signature);
    const editor = new Editor({ extensions: [StarterKit, SignatureBlock], content: original });
    try {
      const text = convertBodyFormat(editor.getHTML(), true, signature);
      expect(removeSignatureFromBody(text, false, signature)).not.toContain("Company");
      editor.commands.setContent(convertBodyFormat(text, false, signature));
      const roundTrip = editor.getHTML();
      expect(roundTrip.match(/data-signature=/g)).toHaveLength(1);
      const replaced = replaceSignatureInBody(roundTrip, "<p>New identity</p>");
      expect(replaced).not.toContain("Company");
      expect(replaced).toContain("Original");
      expect(replaced).toContain("Reply &amp; details");
      expect(replaced.match(/New identity/g)).toHaveLength(1);
    } finally {
      editor.destroy();
    }
  });
  it("preserves the marker after editing so identity changes replace rather than duplicate", () => {
    const editor = new Editor({
      extensions: [StarterKit, SignatureBlock],
      content: injectSignature("<p>Reply</p><blockquote><p>Original</p></blockquote>", "<p>First signature</p>"),
    });
    try {
      editor.commands.insertContentAt(1, "My reply. ");
      const edited = editor.getHTML();
      expect(edited).toContain('data-signature="true"');
      const replaced = replaceSignatureInBody(edited, "<p>Second signature</p>");
      expect(replaced).not.toContain("First signature");
      expect(replaced).toContain("Second signature");
      expect(replaced).toContain("My reply.");
      expect(replaced).toContain("Original");
      expect(replaced.match(/data-signature=/g)).toHaveLength(1);
    } finally {
      editor.destroy();
    }
  });
});
