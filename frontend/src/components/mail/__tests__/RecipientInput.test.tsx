import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createRef } from "react";
import { RecipientInput } from "../ComposeDialog/RecipientInput";

vi.mock("@/hooks/useAutocomplete", () => ({
  useAutocomplete: () => ({ results: [{ item: { email: "alice@example.com", source: "known" }, highlightedEmail: "alice@example.com" }], isLoading: false }),
}));
afterEach(cleanup);

describe("RecipientInput", () => {
  it("uses a growing, wrapping textarea and preserves focus access", () => {
    const ref = createRef<HTMLTextAreaElement>();
    const view = render(<RecipientInput value="" onChange={vi.fn()} inputRef={ref} />);
    const field = screen.getByRole("textbox") as HTMLTextAreaElement;
    expect(field.tagName).toBe("TEXTAREA");
    expect(ref.current).toBe(field);
    Object.defineProperty(field, "scrollHeight", { configurable: true, value: 80 });
    view.rerender(<RecipientInput value="alice@example.com, bob@example.com" onChange={vi.fn()} inputRef={ref} />);
    expect(field.style.height).toBe("80px");
  });

  it("keeps recipient autocomplete and normalizes pasted line breaks", () => {
    const onChange = vi.fn();
    render(<RecipientInput value="bob@example.com, ali" onChange={onChange} />);
    const field = screen.getByRole("textbox");
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("bob@example.com, alice@example.com, ");
    fireEvent.change(field, { target: { value: "alice@example.com,\nbob@example.com" } });
    expect(onChange).toHaveBeenCalledWith("alice@example.com, bob@example.com");
  });
});
