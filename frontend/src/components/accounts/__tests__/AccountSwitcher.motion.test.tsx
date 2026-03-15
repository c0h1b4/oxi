import { fireEvent, render, screen } from "@testing-library/react";
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

const { mockAuthState, mockUiState } = vi.hoisted(() => ({
  mockAuthState: {
    accounts: [
      { id: "a1", email: "admin@example.com", imapHost: "mail.example.com" },
      { id: "a2", email: "ops@example.com", imapHost: "imap.ops.example.com" },
    ],
    activeAccountId: "a1",
    setActiveAccount: vi.fn(),
    removeAccount: vi.fn(),
    setAccounts: vi.fn(),
  },
  mockUiState: {
    effectiveAnimationMode: "medium" as "rich" | "medium" | "subtle" | "off",
  },
}));

vi.mock("@/stores/useAuthStore", () => ({
  useAuthStore: (selector: (state: typeof mockAuthState) => unknown) => selector(mockAuthState),
}));

vi.mock("@/stores/useUiStore", () => ({
  useUiStore: (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState),
}));

vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({ clear: vi.fn() }),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn(), replace: vi.fn() }),
}));

vi.mock("@/lib/api", () => ({
  apiDelete: vi.fn(async () => ({})),
  apiPost: vi.fn(async () => ({})),
}));

vi.mock("../AddAccountModal", () => ({
  AddAccountModal: () => null,
}));

import { AccountSwitcher } from "../AccountSwitcher";

describe("AccountSwitcher motion transitions", () => {
  it("animates dropdown open path in non-off modes", () => {
    mockUiState.effectiveAnimationMode = "medium";
    render(<AccountSwitcher />);

    fireEvent.click(screen.getByRole("button", { name: /admin@example.com/i }));
    expect(screen.getByTestId("account-switcher-dropdown-transition")).toBeTruthy();
  });

  it("keeps static dropdown in off mode", () => {
    mockUiState.effectiveAnimationMode = "off";
    render(<AccountSwitcher />);

    fireEvent.click(screen.getByRole("button", { name: /admin@example.com/i }));
    expect(screen.queryByTestId("account-switcher-dropdown-transition")).toBeNull();
    expect(screen.getByText("Add account")).toBeTruthy();
  });
});
