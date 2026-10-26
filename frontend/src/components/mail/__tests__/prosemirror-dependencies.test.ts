// @vitest-environment node

import { createRequire } from "node:module";
import { describe, expect, it } from "vitest";

// Use native resolution so Vite cannot hide duplicate transitive dependencies.
const require = createRequire(import.meta.url);
const { Schema } = require("@tiptap/pm/model") as typeof import("@tiptap/pm/model");
const { Transform } = require("@tiptap/pm/transform") as typeof import("@tiptap/pm/transform");

describe("ProseMirror dependency compatibility", () => {
  it("splits a paragraph using a node type from the editor schema", () => {
    const schema = new Schema({
      nodes: {
        doc: { content: "paragraph+" },
        paragraph: { content: "text*" },
        text: {},
      },
    });
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("Reply")]),
    ]);
    const transaction = new Transform(doc);

    // Enter's splitBlock supplies typesAfter, passing a Fragment from transform
    // into model's NodeType.create. Separate model copies throw a RangeError.
    transaction.split(6, 1, [{ type: schema.nodes.paragraph }]);

    expect(transaction.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "Reply" }] },
        { type: "paragraph" },
      ],
    });
  });
});
