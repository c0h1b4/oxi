"use client";

import { useState, useCallback, useMemo, useRef } from "react";
import {
  Search,
  UserPlus,
  Users,
  Loader2,
  FolderPlus,
  Upload,
  Download,
  Pencil,
  Trash2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  useContacts,
  useCreateContact,
  useDeleteContact,
  useImportContacts,
} from "@/hooks/useContacts";
import {
  useContactGroups,
  useCreateGroup,
  useDeleteGroup,
  useUpdateGroup,
  useGroupMembers,
} from "@/hooks/useContactGroups";
import { ContactCard, InitialsAvatar } from "@/components/contacts/ContactCard";
import { ContactDialog } from "@/components/contacts/ContactDialog";
import { GroupDialog } from "@/components/contacts/GroupDialog";
import type { Contact } from "@/types/contact";

export function ContactsPanel() {
  const [search, setSearch] = useState("");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [groupDialogOpen, setGroupDialogOpen] = useState(false);
  const [editingGroup, setEditingGroup] = useState<{
    id: string;
    name: string;
  } | null>(null);
  const [selectedContact, setSelectedContact] = useState<Contact | null>(null);
  const [activeGroupId, setActiveGroupId] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const { data, isLoading, error } = useContacts(search || undefined);
  const createContact = useCreateContact();
  const deleteContact = useDeleteContact();
  const importContacts = useImportContacts();

  const { data: groupsData } = useContactGroups();
  const groups = useMemo(() => groupsData?.groups ?? [], [groupsData]);
  const createGroup = useCreateGroup();
  const updateGroup = useUpdateGroup();
  const deleteGroup = useDeleteGroup();
  const { data: groupMembersData } = useGroupMembers(activeGroupId);

  const allContacts = useMemo(() => data?.contacts ?? [], [data]);

  // When a group is selected, show only its members; otherwise show all contacts
  const contacts = useMemo(() => {
    if (activeGroupId && groupMembersData) {
      const memberIds = new Set(groupMembersData.members.map((m) => m.id));
      // If also searching, filter the members list
      if (search) {
        return allContacts.filter((c) => memberIds.has(c.id));
      }
      return groupMembersData.members;
    }
    return allContacts;
  }, [activeGroupId, groupMembersData, allContacts, search]);

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

  const handleCreateGroup = useCallback(
    (name: string) => {
      createGroup.mutate(name, {
        onSuccess: () => setGroupDialogOpen(false),
      });
    },
    [createGroup],
  );

  const handleUpdateGroup = useCallback(
    (name: string) => {
      if (!editingGroup) return;
      updateGroup.mutate(
        { id: editingGroup.id, name },
        {
          onSuccess: () => setEditingGroup(null),
        },
      );
    },
    [updateGroup, editingGroup],
  );

  const handleImport = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (file) {
        importContacts.mutate(file);
        e.target.value = "";
      }
    },
    [importContacts],
  );

  const handleExport = useCallback(() => {
    window.open((process.env.NEXT_PUBLIC_BASE_PATH || "") + "/api/contacts/export", "_blank");
  }, []);

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
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={handleImport}
              title="Import contacts (.vcf)"
              disabled={importContacts.isPending}
            >
              <Upload className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={handleExport}
              title="Export contacts (.vcf)"
            >
              <Download className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => setGroupDialogOpen(true)}
              title="New group"
            >
              <FolderPlus className="size-4" />
            </Button>
            <Button
              size="sm"
              onClick={() => setDialogOpen(true)}
              className="gap-1.5"
            >
              <UserPlus className="size-4" />
              New
            </Button>
          </div>
        </div>

        {/* Hidden file input for import */}
        <input
          ref={fileInputRef}
          type="file"
          accept=".vcf,.vcard,text/vcard"
          className="hidden"
          onChange={handleFileChange}
        />

        {/* Import status */}
        {importContacts.isSuccess && (
          <div className="border-b border-border bg-emerald-500/10 px-4 py-2 text-xs text-emerald-600 dark:text-emerald-400">
            Imported: {importContacts.data.created} created,{" "}
            {importContacts.data.updated} updated,{" "}
            {importContacts.data.skipped} skipped
          </div>
        )}

        {/* Group filter tabs */}
        {groups.length > 0 && (
          <div className="flex flex-wrap gap-1 border-b border-border px-3 py-2">
            <button
              type="button"
              onClick={() => setActiveGroupId(null)}
              className={`rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors ${
                activeGroupId === null
                  ? "bg-primary text-primary-foreground"
                  : "bg-muted text-muted-foreground hover:bg-accent"
              }`}
            >
              All
            </button>
            {groups.map((g) => (
              <div key={g.id} className="group relative flex items-center">
                <button
                  type="button"
                  onClick={() =>
                    setActiveGroupId(activeGroupId === g.id ? null : g.id)
                  }
                  className={`rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors ${
                    activeGroupId === g.id
                      ? "bg-primary text-primary-foreground"
                      : "bg-muted text-muted-foreground hover:bg-accent"
                  }`}
                >
                  {g.name}{" "}
                  <span className="opacity-60">({g.member_count})</span>
                </button>
                <div className="ml-0.5 hidden items-center gap-0.5 group-hover:flex">
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      setEditingGroup({ id: g.id, name: g.name });
                    }}
                    className="rounded p-0.5 text-muted-foreground hover:text-foreground"
                    title="Rename group"
                  >
                    <Pencil className="size-3" />
                  </button>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      if (activeGroupId === g.id) setActiveGroupId(null);
                      deleteGroup.mutate(g.id);
                    }}
                    className="rounded p-0.5 text-muted-foreground hover:text-destructive"
                    title="Delete group"
                  >
                    <Trash2 className="size-3" />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

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
                {search
                  ? "No contacts found"
                  : activeGroupId
                    ? "No contacts in this group"
                    : "No contacts yet"}
              </p>
              {!search && !activeGroupId && (
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
                selectedContact?.id === contact.id ? "bg-accent" : ""
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
              groups={groups}
              activeGroupId={activeGroupId}
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

      {/* Create contact dialog */}
      <ContactDialog
        open={dialogOpen}
        onClose={() => setDialogOpen(false)}
        onSubmit={handleCreate}
        isPending={createContact.isPending}
      />

      {/* Create group dialog */}
      <GroupDialog
        open={groupDialogOpen}
        onClose={() => setGroupDialogOpen(false)}
        onSubmit={handleCreateGroup}
        isPending={createGroup.isPending}
        title="New Group"
      />

      {/* Rename group dialog */}
      <GroupDialog
        open={editingGroup !== null}
        onClose={() => setEditingGroup(null)}
        onSubmit={handleUpdateGroup}
        isPending={updateGroup.isPending}
        initialName={editingGroup?.name ?? ""}
        title="Rename Group"
      />
    </div>
  );
}
