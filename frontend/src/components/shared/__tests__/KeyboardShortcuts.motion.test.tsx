import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

vi.mock("framer-motion", async () => {
  function AnimatePresence({ children }: { children: ReactNode }) {
    return <>{children}</>;
  }

  return {
    AnimatePresence,
    motion: {
      div: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
    },
  };
});

const { mockUiState } = vi.hoisted(() => ({
  mockUiState: {
    shortcutsOpen: true,
    effectiveAnimationMode: "medium" as "rich" | "medium" | "subtle" | "off",
    setShortcutsOpen: vi.fn(),
  },
}));

vi.mock("@/stores/useUiStore", () => ({
  useUiStore: (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState),
}));

import { KeyboardShortcuts } from "../KeyboardShortcuts";

describe("KeyboardShortcuts motion transitions", () => {
  it("animates overlay and content for non-off modes", () => {
    mockUiState.effectiveAnimationMode = "medium";
    render(<KeyboardShortcuts />);

    expect(screen.getByTestId("keyboard-shortcuts-overlay-transition")).toBeTruthy();
    expect(screen.getByTestId("keyboard-shortcuts-content-transition")).toBeTruthy();
  });

  it("falls back to static dialog in off mode", () => {
    mockUiState.effectiveAnimationMode = "off";
    render(<KeyboardShortcuts />);

    expect(screen.queryByTestId("keyboard-shortcuts-overlay-transition")).toBeNull();
    expect(screen.queryByTestId("keyboard-shortcuts-content-transition")).toBeNull();
    expect(screen.getByText("Keyboard Shortcuts")).toBeTruthy();
  });
});
