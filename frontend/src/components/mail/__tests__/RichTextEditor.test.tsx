import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { compile } from "tailwindcss";
import type { Editor } from "@tiptap/react";
import { RichTextEditor } from "../RichTextEditor";

afterEach(cleanup);

async function renderEditor(content: string) {
  const onChange = vi.fn();
  const { container } = render(<RichTextEditor content={content} onChange={onChange} />);
  const editable = await waitFor(() => {
    const element = container.querySelector<HTMLElement & { editor: Editor }>(".tiptap");
    expect(element).not.toBeNull();
    return element!;
  });
  return { editable, editor: editable.editor, onChange };
}

function pressEnter(editable: HTMLElement) {
  fireEvent.keyDown(editable, { key: "Enter", code: "Enter", keyCode: 13 });
}

const signature = '<div data-signature="true"><p>My signature</p></div>';
const quote = "<blockquote><p>Original message</p></blockquote>";

describe("RichTextEditor Enter", () => {
  it.each([
    ["quoted message", quote],
    ["signature and quoted message", signature + quote],
  ])("adds consecutive paragraphs in an empty reply before a %s", async (_name, suffix) => {
    const { editable, editor, onChange } = await renderEditor("<p></p>" + suffix);
    act(() => editor.commands.setTextSelection(1));

    pressEnter(editable);
    expect(editor.getHTML()).toMatch(/^<p><\/p><p><\/p>/);
    pressEnter(editable);
    expect(editor.getHTML()).toMatch(/^<p><\/p><p><\/p><p><\/p>/);
    expect(editable.querySelector("blockquote")?.textContent).toBe("Original message");
    if (suffix.includes("data-signature")) {
      expect(editable.querySelectorAll('[data-signature="true"]')).toHaveLength(1);
      expect(editable.querySelector('[data-signature="true"]')?.textContent).toBe("My signature");
    }
    expect(onChange).toHaveBeenLastCalledWith(editor.getHTML());
  });

  it("splits a reply paragraph without losing the signature or quoted message", async () => {
    const { editable, editor, onChange } = await renderEditor("<p>Reply text</p>" + signature + quote);
    act(() => editor.commands.setTextSelection(6));

    pressEnter(editable);
    expect(editable.querySelectorAll(":scope > p")[0].textContent).toBe("Reply");
    expect(editable.querySelectorAll(":scope > p")[1].textContent).toBe(" text");
    expect(editable.querySelector('[data-signature="true"]')?.textContent).toBe("My signature");
    expect(editable.querySelector("blockquote")?.textContent).toBe("Original message");
    expect(onChange).toHaveBeenLastCalledWith(editor.getHTML());
  });

  it("starts a new paragraph at the end of a reply", async () => {
    const { editable, editor, onChange } = await renderEditor("<p>Reply text</p>" + signature + quote);
    act(() => editor.commands.setTextSelection(11));

    pressEnter(editable);
    expect(editor.getHTML()).toMatch(/^<p>Reply text<\/p><p><\/p>/);
    expect(editable.querySelector('[data-signature="true"]')?.textContent).toBe("My signature");
    expect(editable.querySelector("blockquote")?.textContent).toBe("Original message");
    expect(onChange).toHaveBeenLastCalledWith(editor.getHTML());
  });

  it("inserts a hard break with Shift+Enter", async () => {
    const { editable, editor, onChange } = await renderEditor("<p>Reply text</p>" + signature + quote);
    act(() => editor.commands.setTextSelection(6));

    fireEvent.keyDown(editable, { key: "Enter", code: "Enter", keyCode: 13, shiftKey: true });
    expect(editor.getHTML()).toMatch(/^<p>Reply<br> text<\/p>/);
    expect(editable.querySelector('[data-signature="true"]')?.textContent).toBe("My signature");
    expect(editable.querySelector("blockquote")?.textContent).toBe("Original message");
    expect(onChange).toHaveBeenLastCalledWith(editor.getHTML());
  });

  it("splits a list item with Enter", async () => {
    const { editable, editor, onChange } = await renderEditor("<ul><li><p>First item</p></li></ul>" + quote);
    act(() => editor.commands.setTextSelection(13));

    pressEnter(editable);
    const items = editable.querySelectorAll("li");
    expect(items).toHaveLength(2);
    expect(items[0].textContent).toBe("First item");
    expect(items[1].textContent).toBe("");
    expect(editable.querySelector("blockquote")?.textContent).toBe("Original message");
    expect(onChange).toHaveBeenLastCalledWith(editor.getHTML());
  });
});

describe("RichTextEditor lists", () => {
  it.each([
    ["Bullet List", "ul"],
    ["Ordered List", "ol"],
  ])("creates a list using the %s toolbar button", async (title, tag) => {
    const { editable, editor, onChange } = await renderEditor("<p>First item</p>");
    act(() => editor.commands.setTextSelection(1));

    fireEvent.mouseDown(editable.closest(".flex-col")!.querySelector(`button[title="${title}"]`)!);

    expect(editable.querySelector(`${tag} > li > p`)?.textContent).toBe("First item");
    expect(onChange).toHaveBeenLastCalledWith(editor.getHTML());
  });

  it("restores markers and indentation for both list types at every nesting level", async () => {
    const { editable } = await renderEditor(
      "<ul><li><p>Bullet</p><ol><li><p>Nested number</p></li></ol></li></ul>" +
      "<ol><li><p>Number</p><ul><li><p>Nested bullet</p></li></ul></li></ol>",
    );
    const require = createRequire(import.meta.url);
    const [theme, preflight] = await Promise.all([
      readFile(require.resolve("tailwindcss/theme.css"), "utf8"),
      readFile(require.resolve("tailwindcss/preflight.css"), "utf8"),
    ]);
    const compiler = await compile(`${theme}\n${preflight}\n@tailwind utilities;`);
    const style = document.createElement("style");
    style.textContent = compiler.build(editable.className.split(/\s+/));
    editable.parentElement!.append(style);

    const lists = editable.querySelectorAll("ul, ol");
    expect(lists).toHaveLength(4);
    for (const list of lists) {
      const styles = getComputedStyle(list);
      expect(styles.listStyleType).toBe(list.tagName === "UL" ? "disc" : "decimal");
      expect(styles.paddingInlineStart).not.toBe("");
      expect(styles.paddingInlineStart).not.toBe("0px");
    }
    expect(editable.querySelector("ul > li > ol > li")?.textContent).toBe("Nested number");
    expect(editable.querySelector("ol > li > ul > li")?.textContent).toBe("Nested bullet");
  });
});
