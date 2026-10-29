import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Contact } from "@/types/contact";
import { ContactCard } from "../ContactCard";

const { addMember } = vi.hoisted(() => ({ addMember: vi.fn() }));
vi.mock("@tanstack/react-query", () => ({ useQuery: () => ({ data: [] }) }));
vi.mock("@/hooks/useContactGroups", () => ({
  useAddGroupMember: () => ({ mutate: addMember }),
  useRemoveGroupMember: () => ({ mutate: vi.fn() }),
}));

describe("ContactCard group menu", () => {
  it("can add a contact to a group by tapping rather than hovering", () => {
    const contact: Contact = {
      id: "alice", name: "Alice", email: "alice@example.com", company: "",
      notes: "", source: "manual", created_at: "2026-10-01", updated_at: "2026-10-01",
      last_contacted: null, contact_count: 0, is_favorite: false,
    };
    render(<ContactCard contact={contact} onDelete={vi.fn()} isDeleting={false}
      groups={[{ id: "friends", name: "Friends", member_count: 0, created_at: "2026-10-01", updated_at: "2026-10-01" }]} />);
    const toggle = screen.getByRole("button", { name: "Add to group" });
    expect(screen.queryByRole("button", { name: "Friends" })).toBeNull();
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Friends" }));
    expect(addMember).toHaveBeenCalledWith({ groupId: "friends", contactId: "alice" });
    fireEvent.click(toggle);
    expect(screen.queryByRole("button", { name: "Friends" })).toBeNull();
  });
});
