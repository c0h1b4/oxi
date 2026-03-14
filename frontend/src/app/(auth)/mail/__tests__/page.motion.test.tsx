import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const { mockUiState } = vi.hoisted(() => ({
  mockUiState: {
    viewMode: "mail" as "mail" | "contacts" | "calendar" | "settings",
    effectiveAnimationMode: "medium" as "rich" | "medium" | "subtle" | "off",
  },
}));

vi.mock("@/stores/useUiStore", () => ({
  useUiStore: (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState),
}));

vi.mock("@/hooks/useKeyboardShortcuts", () => ({
  useKeyboardShortcuts: vi.fn(),
}));

vi.mock("@/hooks/useWebSocket", () => ({
  useWebSocket: vi.fn(() => ({ status: "connected", failCount: 0 })),
}));

vi.mock("@/hooks/useNotifications", () => ({
  useNotifications: vi.fn(() => ({
    showBanner: false,
    requestPermission: vi.fn(),
    dismissBanner: vi.fn(),
    handleEvent: vi.fn(),
  })),
}));

vi.mock("@/components/shared/ThreePanelLayout", () => ({
  ThreePanelLayout: vi.fn(() => <div data-testid="mail-layout" />),
}));

vi.mock("@/components/shared/NavRail", () => ({
  NavRail: vi.fn(() => <div data-testid="nav-rail" />),
}));

vi.mock("@/components/mail/FolderTree", () => ({
  FolderTree: vi.fn(() => <div data-testid="folder-tree" />),
}));

vi.mock("@/components/mail/MessageList", () => ({
  MessageList: vi.fn(() => <div data-testid="message-list" />),
}));

vi.mock("@/components/mail/ReadingPane", () => ({
  ReadingPane: vi.fn(() => <div data-testid="reading-pane" />),
}));

vi.mock("@/components/calendar/CalendarPanel", () => ({
  CalendarPanel: vi.fn(() => <div data-testid="calendar-panel" />),
}));

vi.mock("@/components/contacts/ContactsPanel", () => ({
  ContactsPanel: vi.fn(() => <div data-testid="contacts-panel" />),
}));

vi.mock("@/components/settings/SettingsPanel", () => ({
  SettingsPanel: vi.fn(() => <div data-testid="settings-panel" />),
}));

vi.mock("@/components/shared/NotificationBanner", () => ({
  NotificationBanner: vi.fn(() => <div data-testid="notification-banner" />),
}));

vi.mock("@/components/shared/KeyboardShortcuts", () => ({
  KeyboardShortcuts: vi.fn(() => <div data-testid="keyboard-shortcuts" />),
}));

vi.mock("@/components/shared/CommandPalette", () => ({
  CommandPalette: vi.fn(() => <div data-testid="command-palette" />),
}));

vi.mock("@/components/PreferencesLoader", () => ({
  PreferencesLoader: vi.fn(() => <div data-testid="preferences-loader" />),
}));

import MailPage from "../page";

describe("Mail page motion transitions", () => {
  it("uses animated wrappers for all views in non-off modes", () => {
    const views: Array<"mail" | "contacts" | "calendar" | "settings"> = [
      "mail",
      "contacts",
      "calendar",
      "settings",
    ];
    const modes: Array<"rich" | "medium" | "subtle"> = ["rich", "medium", "subtle"];

    for (const mode of modes) {
      mockUiState.effectiveAnimationMode = mode;

      for (const view of views) {
        mockUiState.viewMode = view;
        const { unmount } = render(<MailPage />);
        expect(screen.getByTestId(`mail-view-transition-${view}`)).toBeTruthy();
        unmount();
      }
    }
  });

  it("uses a static non-animated path in off mode", () => {
    const views: Array<"mail" | "contacts" | "calendar" | "settings"> = [
      "mail",
      "contacts",
      "calendar",
      "settings",
    ];

    mockUiState.effectiveAnimationMode = "off";

    for (const view of views) {
      mockUiState.viewMode = view;
      const { unmount } = render(<MailPage />);
      expect(screen.getByTestId(`mail-view-static-${view}`)).toBeTruthy();
      expect(screen.queryByTestId(`mail-view-transition-${view}`)).toBeNull();
      unmount();
    }
  });
});
