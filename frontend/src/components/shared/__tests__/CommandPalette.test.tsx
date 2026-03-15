import { render, screen, fireEvent, act } from "@testing-library/react";
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
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

vi.mock("radix-ui", () => ({
  Dialog: {
    Root: ({ children }: { children: ReactNode }) => <>{children}</>,
    Portal: ({ children }: { children: ReactNode }) => <>{children}</>,
    Overlay: ({ children }: { children: ReactNode }) => <>{children}</>,
    Content: ({ children }: { children: ReactNode }) => <>{children}</>,
  },
}));

vi.mock("cmdk", () => {
  function CommandRoot({ children }: { children: ReactNode }) {
    return <div>{children}</div>;
  }

  const CommandInput = ({ ...props }: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props} />;
  CommandInput.displayName = "MockCommandInput";

  const CommandList = ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>;
  CommandList.displayName = "MockCommandList";

  const CommandEmpty = ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>;
  CommandEmpty.displayName = "MockCommandEmpty";

  const CommandGroup = ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>;
  CommandGroup.displayName = "MockCommandGroup";

  const CommandItem = ({ children, onSelect, ...props }: React.HTMLAttributes<HTMLButtonElement> & { onSelect?: () => void }) => (
    <button type="button" onClick={onSelect} {...props}>{children}</button>
  );
  CommandItem.displayName = "MockCommandItem";

  CommandRoot.Input = CommandInput;
  CommandRoot.List = CommandList;
  CommandRoot.Empty = CommandEmpty;
  CommandRoot.Group = CommandGroup;
  CommandRoot.Item = CommandItem;

  return { Command: CommandRoot };
});

const { mockUiState, mockComposeState } = vi.hoisted(() => ({
  mockUiState: {
    commandPaletteOpen: true,
    theme: "light" as "light" | "dark" | "system",
    effectiveAnimationMode: "off" as "rich" | "medium" | "subtle" | "off",
    setCommandPaletteOpen: vi.fn(),
    setTheme: vi.fn(),
    setViewMode: vi.fn(),
    setActiveFolder: vi.fn(),
    setSearchActive: vi.fn(),
    setShortcutsOpen: vi.fn(),
  },
  mockComposeState: {
    openCompose: vi.fn(),
  },
}));

vi.mock("@/stores/useUiStore", () => {
  const useUiStore = (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState);
  useUiStore.getState = () => mockUiState;
  return { useUiStore };
});

vi.mock("@/stores/useComposeStore", () => ({
  useComposeStore: {
    getState: () => mockComposeState,
  },
}));

vi.mock("@/hooks/useDisplayPreferences", () => ({
  useUpdateDisplayPreferences: () => ({ mutate: vi.fn() }),
}));

vi.mock("@/lib/motion/theme-spread", () => ({
  runThemeSpreadTransition: ({ applyTheme }: { applyTheme: () => void }) => applyTheme(),
}));

import { CommandPalette } from "../CommandPalette";

describe("CommandPalette", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    document.body.innerHTML = "";
    mockUiState.setSearchActive.mockClear();
    mockUiState.setCommandPaletteOpen.mockClear();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it("search emails focuses input without search mode", () => {
    const searchInput = document.createElement("input");
    searchInput.setAttribute("data-search-input", "");
    document.body.appendChild(searchInput);

    render(<CommandPalette />);

    fireEvent.click(screen.getByRole("button", { name: "Search emails" }));

    act(() => {
      vi.runAllTimers();
    });

    expect(document.activeElement).toBe(searchInput);
    expect(mockUiState.setSearchActive).not.toHaveBeenCalled();
    expect(mockUiState.setCommandPaletteOpen).toHaveBeenCalledWith(false);
  });
});
