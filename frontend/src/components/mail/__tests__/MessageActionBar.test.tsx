import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MessageRemoval } from "@/hooks/pending-message-removals";

const state = vi.hoisted(() => ({
  ui: { activeFolder: "INBOX", selectedMessageUid: 1 as number | null, effectiveAnimationMode: "off" },
  removals: [] as MessageRemoval[],
  move: vi.fn(), delete: vi.fn(), message: vi.fn(), moveOptions: vi.fn(), deleteOptions: vi.fn(),
}));
vi.mock("@/stores/useUiStore", () => ({ useUiStore: (select: (value: typeof state.ui) => unknown) => select(state.ui) }));
vi.mock("@/hooks/useMessages", () => ({
  useMessage: state.message,
  useUpdateFlags: () => ({ mutate: vi.fn() }),
  useMoveMessage: (options: unknown) => { state.moveOptions(options); return { mutate: state.move }; },
  useDeleteMessage: (options: unknown) => { state.deleteOptions(options); return { mutate: state.delete }; },
}));
vi.mock("@/hooks/pending-message-removals", () => ({ useMessageRemovals: () => state.removals }));
vi.mock("@/hooks/useFolders", () => ({ useFolders: () => ({ data: { folders: [{ name: "Junk" }] } }) }));
vi.mock("@/hooks/useIdentities", () => ({ useIdentities: () => ({ data: [] }) }));
vi.mock("@/hooks/useI18n", () => ({ useLocale: () => "en" }));
vi.mock("@/stores/useAuthStore", () => ({ useAuthStore: { getState: () => ({ activeAccount: () => ({ email: "me@example.com" }) }) } }));
vi.mock("../TagPicker", () => ({ TagPicker: () => null }));
vi.mock("../MoveToFolderMenu", () => ({ MoveToFolderMenu: ({ onMove }: { onMove: (folder: string) => void }) => <button aria-label="Move to folder" onClick={() => onMove("Projects")}>Move</button> }));

import { MessageActionBar } from "../MessageActionBar";
const button = (name: string) => screen.getByRole("button", { name }) as HTMLButtonElement;

beforeEach(() => {
  vi.clearAllMocks();
  state.ui.activeFolder = "INBOX";
  state.ui.selectedMessageUid = 1;
  state.removals = [];
  state.message.mockReturnValue({ data: undefined });
});
afterEach(cleanup);

describe("MessageActionBar confirmation actions", () => {
  it.each([["Delete", "Trash"], ["Archive", "Archive"], ["Junk", "Junk"], ["Move to folder", "Projects"]])("%s uses the selected message without waiting for its body", (label, toFolder) => {
    render(<MessageActionBar />);
    expect(button("Reply").disabled).toBe(true);
    expect(button("Forward").disabled).toBe(true);
    fireEvent.click(button(label));
    expect(state.move).toHaveBeenCalledWith({ fromFolder: "INBOX", toFolder, uid: 1 });
    expect(state.moveOptions).toHaveBeenCalledWith({ confirmation: true });
  });

  it("does not target a stale body UID", () => {
    state.message.mockReturnValue({ data: {
      uid: 99, folder: "Other", flags: [], to_addresses: [], cc_addresses: [], from_address: "sender@example.com",
    } });
    render(<MessageActionBar />);
    fireEvent.click(button("Delete"));
    expect(state.move).toHaveBeenCalledWith({ fromFolder: "INBOX", toFolder: "Trash", uid: 1 });
  });

  it("permanently deletes the selected Trash UID without a body", () => {
    state.ui.activeFolder = "Trash";
    render(<MessageActionBar />);
    fireEvent.click(button("Delete"));
    expect(state.delete).toHaveBeenCalledWith({ folder: "Trash", uid: 1 });
    expect(state.deleteOptions).toHaveBeenCalledWith({ confirmation: true });
  });

  it.each([["Delete", "Trash"], ["Archive", "Archive"], ["Junk", "Junk"], ["Move to folder", "Projects"], ["Delete", undefined]])("restores %s feedback on remount and disables conflicting actions only for its message (%s)", (label, toFolder) => {
    state.removals = [{ fromFolder: "INBOX", uid: 1, toFolder, accountId: "a", cancelled: false, removed: false, confirmation: true }];
    const view = render(<MessageActionBar />);
    expect(button(label).getAttribute("aria-busy")).toBe("true");
    expect(button(label).querySelector(".animate-spin")).not.toBeNull();
    for (const action of ["Delete", "Archive", "Junk", "Move to folder"]) {
      expect(button(action).disabled).toBe(true);
      fireEvent.click(button(action));
    }
    expect(state.move).not.toHaveBeenCalled();
    expect(state.delete).not.toHaveBeenCalled();
    state.ui.selectedMessageUid = 2;
    view.rerender(<MessageActionBar />);
    expect(button("Delete").disabled).toBe(false);
    expect(view.container.querySelector(".animate-spin")).toBeNull();
    state.ui.selectedMessageUid = 1;
    state.ui.activeFolder = "Other";
    view.rerender(<MessageActionBar />);
    expect(button("Delete").disabled).toBe(false);
    view.unmount();
    state.ui.activeFolder = "INBOX";
    render(<MessageActionBar />);
    expect(button(label).getAttribute("aria-busy")).toBe("true");
    expect(button(label).disabled).toBe(true);
  });

  it("disables removal actions when nothing is selected", () => {
    state.ui.selectedMessageUid = null;
    render(<MessageActionBar />);
    for (const action of ["Delete", "Archive", "Junk", "Move to folder"]) expect(button(action).disabled).toBe(true);
  });
});
