import { describe, expect, it } from "vitest";
import { Editor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { SignatureBlock } from "../SignatureBlock";
import { injectSignature, replaceSignatureInBody } from "@/stores/useComposeStore";

describe("signature editor round-trip", () => {
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
