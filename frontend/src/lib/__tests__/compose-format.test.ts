import { describe, expect, it } from "vitest";
import { htmlToPlainText, plainTextToHtml } from "../compose-format";
import { convertBodyFormat, injectSignature, removeSignatureFromBody, replaceSignatureInBody } from "@/stores/useComposeStore";

describe("compose format conversion", () => {
  it("keeps paragraph, break and quote boundaries readable", () => {
    expect(htmlToPlainText("<p>First<br>Second</p><p>Third</p><blockquote><p>Original</p><p>Another line</p></blockquote>"))
      .toBe("First\nSecond\nThird\n> Original\n> Another line");
    expect(plainTextToHtml("<img src=x> & literal\n> original"))
      .toBe("<p>&lt;img src=x&gt; &amp; literal</p><blockquote><p>original</p></blockquote>");
  });

  it("keeps multiline signatures removable at the end of a body", () => {
    const signature = "<div><p>Me</p><div>Company<br>Phone</div></div>";
    const html = injectSignature("<p>My reply</p>", signature);
    const text = convertBodyFormat(html, true, signature);
    expect(text).toContain("-- \nMe\nCompany\nPhone");
    expect(removeSignatureFromBody(text, false, signature)).toBe("My reply\n");
    const back = convertBodyFormat(text, false, signature);
    expect(back.match(/data-signature=/g)).toHaveLength(1);
    expect(removeSignatureFromBody(back)).not.toContain("Company");
  });

  it("does not remove a quoted copy of the sender's signature", () => {
    const signature = "<p>Me</p>";
    const text = "> Original\n> -- \n> Me";
    expect(removeSignatureFromBody(text, false, signature)).toBe(text);
    const replaced = replaceSignatureInBody(text, "<p>New</p>", false, signature);
    expect(replaced.indexOf("New")).toBeLessThan(replaced.indexOf("> Original"));
    expect(replaced).toContain("> Me");
  });
});
