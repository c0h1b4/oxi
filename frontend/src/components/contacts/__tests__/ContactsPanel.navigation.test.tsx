import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Contact } from "@/types/contact";
import { useUiStore } from "@/stores/useUiStore";

const { contact, mutate } = vi.hoisted(() => ({
  contact: { id: "alice", name: "Alice", email: "alice@example.com" },
  mutate: vi.fn(),
}));
vi.mock("@/hooks/useContacts", () => ({
  useContacts: () => ({ data: { contacts: [contact], total: 1 }, isLoading: false }),
  useCreateContact: () => ({ mutate }),
  useDeleteContact: () => ({ mutate }),
  useImportContacts: () => ({ mutate }),
}));
vi.mock("@/hooks/useContactGroups", () => ({
  useContactGroups: () => ({ data: { groups: [] } }),
  useCreateGroup: () => ({ mutate }),
  useDeleteGroup: () => ({ mutate }),
  useUpdateGroup: () => ({ mutate }),
  useGroupMembers: () => ({ data: null }),
}));
vi.mock("@/components/contacts/ContactCard", () => ({
  InitialsAvatar: () => null,
  ContactCard: ({ contact, onDelete }: { contact: Contact; onDelete: (id: string) => void }) => (
    <section aria-label="Contact details">
      <h2>{contact.name}</h2>
      <button onClick={() => onDelete(contact.id)}>Delete contact</button>
    </section>
  ),
}));
vi.mock("@/components/contacts/ContactDialog", () => ({ ContactDialog: () => null }));
vi.mock("@/components/contacts/GroupDialog", () => ({ GroupDialog: () => null }));
import { ContactsPanel } from "../ContactsPanel";

describe("Contacts list and detail navigation", () => {
  beforeEach(() => {
    useUiStore.setState({ effectiveAnimationMode: "off" });
    mutate.mockReset();
  });

  it("opens details and returns to the list without clearing the search", () => {
    render(<ContactsPanel />);
    const search = screen.getByPlaceholderText("Search contacts...");
    fireEvent.change(search, { target: { value: "Alice" } });
    fireEvent.click(screen.getByRole("button", { name: "Alice alice@example.com" }));
    expect(screen.getByRole("region", { name: "Contact details" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Back to contacts" }));
    expect(screen.queryByRole("region", { name: "Contact details" })).toBeNull();
    expect((search as HTMLInputElement).value).toBe("Alice");
    expect(screen.getByRole("button", { name: "Alice alice@example.com" })).toBeTruthy();
  });

  it("returns to the list after deleting the selected contact", () => {
    mutate.mockImplementation((_id: string, options: { onSuccess: () => void }) => options.onSuccess());
    render(<ContactsPanel />);
    fireEvent.click(screen.getByRole("button", { name: "Alice alice@example.com" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete contact" }));
    expect(mutate).toHaveBeenCalledWith("alice", expect.any(Object));
    expect(screen.queryByRole("button", { name: "Back to contacts" })).toBeNull();
  });
});
