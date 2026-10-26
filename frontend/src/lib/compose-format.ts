/** Convert editor blocks to text without joining paragraphs or quoted lines. */
export function htmlToPlainText(html: string): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  function text(node: Node): string {
    if (node.nodeType === Node.TEXT_NODE) return node.textContent ?? "";
    if (!(node instanceof Element)) return Array.from(node.childNodes).map(text).join("");
    if (node.tagName === "BR") return "\n";
    const content = Array.from(node.childNodes).map(text).join("");
    if (node.tagName === "BLOCKQUOTE") {
      return content.replace(/\n+$/, "").split("\n").map(line => `> ${line}`).join("\n") + "\n";
    }
    return /^(P|DIV|LI|H[1-6]|PRE|TR)$/.test(node.tagName)
      ? content.replace(/\n+$/, "") + "\n"
      : content;
  }
  return text(doc.body).replace(/\n+$/, "");
}

/** Escape literal text before putting it back in the rich-text editor. */
export function plainTextToHtml(text: string): string {
  const escape = (line: string) => line.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  let inQuote = false;
  const blocks: string[] = [];
  for (const line of text.split("\n")) {
    const quoted = line.startsWith("> ") || line === ">";
    if (quoted && !inQuote) blocks.push("<blockquote>");
    if (!quoted && inQuote) blocks.push("</blockquote>");
    inQuote = quoted;
    blocks.push(`<p>${escape(quoted ? line.replace(/^> ?/, "") : line) || "<br>"}</p>`);
  }
  if (inQuote) blocks.push("</blockquote>");
  return blocks.join("");
}
