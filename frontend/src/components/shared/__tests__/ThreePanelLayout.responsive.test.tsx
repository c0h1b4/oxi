import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useUiStore } from "@/stores/useUiStore";
import { ThreePanelLayout } from "../ThreePanelLayout";

vi.mock("@/components/mail/SearchBar", () => ({ SearchBar: () => <input aria-label="Search mail" /> }));
vi.mock("@/components/mail/SearchResults", () => ({ SearchResults: () => <button onClick={() => useUiStore.getState().selectMessage(7)}>Search result</button> }));
vi.mock("@/components/mail/MessageActionBar", () => ({ MessageActionBar: () => <button>Reply</button> }));

let viewportWidth = 320;
let listeners: Set<() => void>;
const initialState = useUiStore.getState();

function resize(width: number) {
  act(() => {
    viewportWidth = width;
    listeners.forEach((listener) => listener());
  });
}

function renderLayout() {
  return render(<ThreePanelLayout
    sidebar={<button onClick={() => useUiStore.getState().setActiveFolder("INBOX")}>Inbox folder</button>}
    messageList={<button onClick={() => useUiStore.getState().selectMessage(7)}>Open message</button>}
    readingPane={<div>Message body</div>}
  />);
}

beforeEach(() => {
  viewportWidth = 320;
  listeners = new Set();
  useUiStore.setState({ ...initialState, effectiveAnimationMode: "off", sidebarOpen: false, selectedMessageUid: null });
  vi.stubGlobal("matchMedia", vi.fn((query: string) => ({
    get matches() { return viewportWidth >= Number(query.match(/min-width: (\d+)px/)?.[1]); },
    media: query,
    addEventListener: (_event: string, listener: () => void) => listeners.add(listener),
    removeEventListener: (_event: string, listener: () => void) => listeners.delete(listener),
  })));
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  useUiStore.setState(initialState);
});

describe("responsive mail navigation", () => {
  it.each([320, 768])("opens messages and returns to the retained list at %ipx", async (width) => {
    viewportWidth = width;
    renderLayout();
    const list = screen.getByRole("main", { name: "Messages" });
    list.scrollTop = 120;
    expect(screen.queryByText("Message body")).toBeNull();
    expect(screen.queryByRole("button", { name: "Resize panel" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open message" }));
    expect(screen.getByText("Message body")).toBeTruthy();
    expect(screen.queryByRole("main", { name: "Messages" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Back to messages" }));
    expect(screen.getByRole("main", { name: "Messages" })).toBe(list);
    expect(list.scrollTop).toBe(120);
    await waitFor(() => expect(document.activeElement).toBe(list));
  });

  it("keeps search state when returning from a result", () => {
    useUiStore.setState({ searchActive: true, searchQuery: "from:alice" });
    renderLayout();
    fireEvent.click(screen.getByRole("button", { name: "Search result" }));
    fireEvent.click(screen.getByRole("button", { name: "Back to results" }));
    expect(screen.getByRole("button", { name: "Search result" })).toBeTruthy();
    expect(useUiStore.getState().searchQuery).toBe("from:alice");
  });

  it("opens and closes the folder drawer and restores focus", async () => {
    renderLayout();
    const trigger = screen.getByRole("button", { name: "Open folders" });
    fireEvent.click(trigger);
    expect(screen.getByRole("dialog", { name: "Folders" })).toBeTruthy();
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(trigger));
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole("button", { name: "Inbox folder" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(useUiStore.getState().sidebarOpen).toBe(false);
  });

  it("uses two panes on tablets and resizable three panes on desktop", () => {
    viewportWidth = 1024;
    useUiStore.setState({ selectedMessageUid: 7 });
    renderLayout();
    expect(screen.getByRole("main", { name: "Messages" })).toBeTruthy();
    expect(screen.getByText("Message body")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Resize panel" })).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Open folders" })).toBeTruthy();
    resize(1440);
    expect(screen.queryByRole("button", { name: "Open folders" })).toBeNull();
    expect(screen.getByRole("complementary", { name: "Folders" })).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Resize panel" })).toHaveLength(2);
  });

  it("opens mobile messages without overwriting the desktop reading-pane preference or panel sizes", () => {
    viewportWidth = 1440;
    useUiStore.setState({ readingPaneVisible: false, sidebarWidth: 240, messageListWidth: 500 });
    renderLayout();
    resize(320);
    fireEvent.click(screen.getByRole("button", { name: "Open message" }));
    expect(screen.getByText("Message body")).toBeTruthy();
    resize(1440);
    expect(screen.queryByText("Message body")).toBeNull();
    expect(useUiStore.getState()).toMatchObject({ readingPaneVisible: false, sidebarWidth: 240, messageListWidth: 500 });
  });

  it("closes the drawer when resizing to desktop", () => {
    renderLayout();
    fireEvent.click(screen.getByRole("button", { name: "Open folders" }));
    resize(1440);
    expect(screen.queryByRole("dialog")).toBeNull();
    resize(320);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
