import { Node } from "@tiptap/react";

/** Preserve the signature marker when the editor parses and serializes the body. */
export const SignatureBlock = Node.create({
  name: "signatureBlock",
  group: "block",
  content: "block+",
  defining: true,
  parseHTML: () => [{ tag: 'div[data-signature="true"]' }],
  renderHTML: () => ["div", { class: "email-signature", "data-signature": "true" }, 0],
});
