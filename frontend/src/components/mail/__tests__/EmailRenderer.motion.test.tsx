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

function expectAnimationStyle(element: HTMLElement, expectedName?: string) {
  const animationName = element.style.animationName;
  const animationDuration = element.style.animationDuration;

  expect(animationName).not.toBe("");
  expect(animationName).not.toBe("none");
  expect(animationDuration).not.toBe("");
  expect(animationDuration).not.toBe("0ms");

  if (expectedName) {
    expect(animationName).toBe(expectedName);
  }
}

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

    const stream = screen.getByTestId("email-renderer-plaintext-rich-stream");
    const lines = screen.getAllByTestId("email-renderer-plaintext-line");

    expect(stream).toBeTruthy();
    expect(lines).toHaveLength(2);
    expectAnimationStyle(lines[0] as HTMLElement, "email-plaintext-line-reveal");
    expect((lines[0] as HTMLElement).style.animationDelay).toBe("0ms");
    expect((lines[1] as HTMLElement).style.animationDelay).toBe("18ms");
  });

  it("streams plaintext in rich mode when switching from html to plaintext", () => {
    mockUiState.effectiveAnimationMode = "rich";

    const { rerender } = render(<EmailRenderer html="<p>Hello</p>" text="hello" />);
    rerender(<EmailRenderer html={null} text={"hello\nagain"} />);

    const stream = screen.getByTestId("email-renderer-plaintext-rich-stream");
    const lines = screen.getAllByTestId("email-renderer-plaintext-line");

    expect(stream).toBeTruthy();
    expectAnimationStyle(lines[0] as HTMLElement, "email-plaintext-line-reveal");
  });

  it("uses simpler transition in medium and subtle modes", () => {
    mockUiState.effectiveAnimationMode = "medium";
    const { rerender } = render(<EmailRenderer html={null} text="medium" />);

    const medium = screen.getByTestId("email-renderer-plaintext-simple-transition");
    expectAnimationStyle(medium as HTMLElement, "email-plaintext-medium-reveal");

    const mediumDuration = (medium as HTMLElement).style.animationDuration;

    mockUiState.effectiveAnimationMode = "subtle";
    rerender(<EmailRenderer html={null} text="subtle" />);

    const subtle = screen.getByTestId("email-renderer-plaintext-simple-transition");
    expectAnimationStyle(subtle as HTMLElement, "email-plaintext-subtle-reveal");
    expect((subtle as HTMLElement).style.animationDuration).not.toBe(mediumDuration);
  });

  it("renders instantly in off mode", () => {
    mockUiState.effectiveAnimationMode = "off";

    render(<EmailRenderer html={null} text="instant" />);

    const instant = screen.getByTestId("email-renderer-plaintext-static");
    expect(instant).toBeTruthy();
    expect((instant as HTMLElement).style.animationName).toBe("");
    expect((instant as HTMLElement).style.animationDuration).toBe("");
    expect(screen.queryByTestId("email-renderer-plaintext-rich-stream")).toBeNull();
    expect(screen.queryByTestId("email-renderer-plaintext-simple-transition")).toBeNull();
  });

  it("uses single-container reveal for large plaintext bodies in rich mode", () => {
    mockUiState.effectiveAnimationMode = "rich";
    const largeText = Array.from({ length: 220 }, (_, idx) => `line ${idx + 1}`).join("\n");

    render(<EmailRenderer html={null} text={largeText} />);

    const largeReveal = screen.getByTestId("email-renderer-plaintext-large-reveal");
    expect(largeReveal).toBeTruthy();
    expectAnimationStyle(largeReveal as HTMLElement, "email-plaintext-container-reveal");
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

    const lines = screen.getAllByTestId("email-renderer-plaintext-line");
    expectAnimationStyle(lines[0] as HTMLElement, "email-plaintext-line-reveal");
  });
});
