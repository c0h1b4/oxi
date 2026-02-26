"use client";

import { useState, useCallback, useMemo } from "react";
import { Search, UserPlus, Users, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useContacts, useCreateContact, useDeleteContact } from "@/hooks/useContacts";
import { ContactCard, InitialsAvatar } from "@/components/contacts/ContactCard";
import { ContactDialog } from "@/components/contacts/ContactDialog";
import type { Contact } from "@/types/contact";

export function ContactsPanel() {
  const [search, setSearch] = useState("");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [selectedContact, setSelectedContact] = useState<Contact | null>(null);

  // Debounced search: use the search value directly, the query will re-run
  const { data, isLoading, error } = useContacts(search || undefined);
  const createContact = useCreateContact();
  const deleteContact = useDeleteContact();

  const contacts = useMemo(() => data?.contacts ?? [], [data]);

  const handleCreate = useCallback(
    (formData: {
      name: string;
      email: string;
      company?: string;
      notes?: string;
    }) => {
      createContact.mutate(
        { ...formData },
        {
          onSuccess: () => {
            setDialogOpen(false);
          },
        },
      );
    },
    [createContact],
  );

  const handleDelete = useCallback(
    (id: string) => {
      deleteContact.mutate(id, {
        onSuccess: () => {
          if (selectedContact?.id === id) {
            setSelectedContact(null);
          }
        },
      });
    },
    [deleteContact, selectedContact],
  );

  return (
    <div className="flex h-full min-w-0 flex-1">
      {/* Contact list */}
      <div className="flex w-[360px] shrink-0 flex-col border-r border-border">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <div className="flex items-center gap-2">
            <Users className="size-5 text-primary" />
            <h1 className="text-base font-semibold text-foreground">
              Contacts
            </h1>
            {data && (
              <span className="text-xs text-muted-foreground">
                ({data.total})
              </span>
            )}
          </div>
          <Button
            size="sm"
            onClick={() => setDialogOpen(true)}
            className="gap-1.5"
          >
            <UserPlus className="size-4" />
            New
          </Button>
        </div>

        {/* Search */}
        <div className="border-b border-border px-3 py-2">
          <div className="relative">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <input
              type="text"
              placeholder="Search contacts..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="h-8 w-full rounded-md border border-input bg-transparent pl-8 pr-3 text-sm placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] outline-none dark:bg-input/30"
            />
          </div>
        </div>

        {/* Contact list */}
        <div className="flex-1 overflow-y-auto">
          {isLoading && (
            <div className="flex items-center justify-center py-12">
              <Loader2 className="size-5 animate-spin text-muted-foreground" />
            </div>
          )}

          {error && (
            <div className="px-4 py-8 text-center text-sm text-destructive">
              Failed to load contacts
            </div>
          )}

          {!isLoading && !error && contacts.length === 0 && (
            <div className="flex flex-col items-center justify-center gap-2 px-4 py-12">
              <Users className="size-10 text-muted-foreground/40" />
              <p className="text-sm text-muted-foreground">
                {search ? "No contacts found" : "No contacts yet"}
              </p>
              {!search && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setDialogOpen(true)}
                  className="mt-2 gap-1.5"
                >
                  <UserPlus className="size-4" />
                  Add your first contact
                </Button>
              )}
            </div>
          )}

          {contacts.map((contact) => (
            <button
              key={contact.id}
              type="button"
              onClick={() => setSelectedContact(contact)}
              className={`flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-accent ${
                selectedContact?.id === contact.id
                  ? "bg-accent"
                  : ""
              }`}
            >
              <InitialsAvatar
                name={contact.name}
                email={contact.email}
                size="sm"
              />
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-medium text-foreground">
                  {contact.name || contact.email}
                </div>
                {contact.name && (
                  <div className="truncate text-xs text-muted-foreground">
                    {contact.email}
                  </div>
                )}
                {contact.company && (
                  <div className="truncate text-xs text-muted-foreground/70">
                    {contact.company}
                  </div>
                )}
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* Detail pane */}
      <div className="flex min-w-0 flex-1 items-center justify-center">
        {selectedContact ? (
          <div className="w-full max-w-lg p-6">
            <ContactCard
              contact={selectedContact}
              onDelete={handleDelete}
              isDeleting={deleteContact.isPending}
            />
          </div>
        ) : (
          <div className="flex flex-col items-center gap-2">
            <Users className="size-12 text-muted-foreground/30" />
            <p className="text-sm text-muted-foreground">
              Select a contact to view details
            </p>
          </div>
        )}
      </div>

      {/* Create dialog */}
      <ContactDialog
        open={dialogOpen}
        onClose={() => setDialogOpen(false)}
        onSubmit={handleCreate}
        isPending={createContact.isPending}
      />
    </div>
  );
}
