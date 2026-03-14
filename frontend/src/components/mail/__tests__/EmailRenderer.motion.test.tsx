import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockUiState } = vi.hoisted(() => ({
  mockUiState: {
    theme: "light" as "light" | "dark" | "system",
    effectiveAnimationMode: "medium" as "rich" | "medium" | "subtle" | "off",
  },
}));

vi.mock("@/stores/useUiStore", () => ({
  useUiStore: (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState),
}));

import { EmailRenderer } from "../EmailRenderer";

describe("EmailRenderer plaintext motion", () => {
  beforeEach(() => {
    mockUiState.theme = "light";
    mockUiState.effectiveAnimationMode = "medium";

    if (!window.matchMedia) {
      Object.defineProperty(window, "matchMedia", {
        writable: true,
        value: vi.fn().mockImplementation(() => ({
          matches: false,
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
        })),
      });
    }
  });

  it("streams plaintext in rich mode on first plaintext render", () => {
    mockUiState.effectiveAnimationMode = "rich";

    render(<EmailRenderer html={null} text={"first\nsecond"} />);

    expect(screen.getByTestId("email-renderer-plaintext-rich-stream")).toBeTruthy();
    expect(screen.getAllByTestId("email-renderer-plaintext-line")).toHaveLength(2);
  });

  it("streams plaintext in rich mode when switching from html to plaintext", () => {
    mockUiState.effectiveAnimationMode = "rich";

    const { rerender } = render(<EmailRenderer html="<p>Hello</p>" text="hello" />);
    rerender(<EmailRenderer html={null} text={"hello\nagain"} />);

    expect(screen.getByTestId("email-renderer-plaintext-rich-stream")).toBeTruthy();
  });

  it("uses simpler transition in medium and subtle modes", () => {
    mockUiState.effectiveAnimationMode = "medium";
    const { rerender } = render(<EmailRenderer html={null} text="medium" />);

    expect(screen.getByTestId("email-renderer-plaintext-simple-transition")).toBeTruthy();

    mockUiState.effectiveAnimationMode = "subtle";
    rerender(<EmailRenderer html={null} text="subtle" />);

    expect(screen.getByTestId("email-renderer-plaintext-simple-transition")).toBeTruthy();
  });

  it("renders instantly in off mode", () => {
    mockUiState.effectiveAnimationMode = "off";

    render(<EmailRenderer html={null} text="instant" />);

    expect(screen.getByTestId("email-renderer-plaintext-static")).toBeTruthy();
    expect(screen.queryByTestId("email-renderer-plaintext-rich-stream")).toBeNull();
    expect(screen.queryByTestId("email-renderer-plaintext-simple-transition")).toBeNull();
  });

  it("uses single-container reveal for large plaintext bodies in rich mode", () => {
    mockUiState.effectiveAnimationMode = "rich";
    const largeText = Array.from({ length: 220 }, (_, idx) => `line ${idx + 1}`).join("\n");

    render(<EmailRenderer html={null} text={largeText} />);

    expect(screen.getByTestId("email-renderer-plaintext-large-reveal")).toBeTruthy();
    expect(screen.queryByTestId("email-renderer-plaintext-line")).toBeNull();
  });

  it("cancels active stream session on message or surface change", () => {
    mockUiState.effectiveAnimationMode = "rich";

    const { rerender } = render(<EmailRenderer html={null} text={"first\nmessage"} />);
    const initial = screen.getByTestId("email-renderer-plaintext-rich-stream").getAttribute("data-stream-session");

    rerender(<EmailRenderer html={null} text={"second\nmessage"} />);
    const afterMessageChange = screen
      .getByTestId("email-renderer-plaintext-rich-stream")
      .getAttribute("data-stream-session");

    expect(afterMessageChange).not.toBe(initial);

    rerender(<EmailRenderer html="<p>html</p>" text="second\nmessage" />);
    rerender(<EmailRenderer html={null} text={"third\nmessage"} />);

    const afterSurfaceChange = screen
      .getByTestId("email-renderer-plaintext-rich-stream")
      .getAttribute("data-stream-session");

    expect(afterSurfaceChange).not.toBe(afterMessageChange);
  });
});
