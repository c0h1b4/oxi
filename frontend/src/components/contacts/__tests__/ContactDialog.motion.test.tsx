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
    effectiveAnimationMode: "medium" as "rich" | "medium" | "subtle" | "off",
  },
}));

vi.mock("@/stores/useUiStore", () => ({
  useUiStore: (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState),
}));

import { ContactDialog } from "../ContactDialog";

describe("ContactDialog motion transitions", () => {
  it("animates dialog overlay/content in non-off modes", () => {
    mockUiState.effectiveAnimationMode = "medium";
    render(
      <ContactDialog
        open
        onClose={vi.fn()}
        onSubmit={vi.fn()}
        isPending={false}
      />,
    );

    expect(screen.getByTestId("contact-dialog-overlay-transition")).toBeTruthy();
    expect(screen.getByTestId("contact-dialog-content-transition")).toBeTruthy();
  });

  it("uses static dialog path when mode is off", () => {
    mockUiState.effectiveAnimationMode = "off";
    render(
      <ContactDialog
        open
        onClose={vi.fn()}
        onSubmit={vi.fn()}
        isPending={false}
      />,
    );

    expect(screen.queryByTestId("contact-dialog-overlay-transition")).toBeNull();
    expect(screen.queryByTestId("contact-dialog-content-transition")).toBeNull();
    expect(screen.getByText("New Contact")).toBeTruthy();
  });
});
