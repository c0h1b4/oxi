"use client";

import { Trash2, Building2, Mail, StickyNote, Clock, Tag, Users, Plus, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { Contact, ContactGroup } from "@/types/contact";
import {
  useAddGroupMember,
  useRemoveGroupMember,
} from "@/hooks/useContactGroups";
import { useQuery } from "@tanstack/react-query";
import { apiGet } from "@/lib/api";

function formatDate(dateStr: string): string {
  try {
    return new Date(dateStr).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return dateStr;
  }
}

function InitialsAvatar({
  name,
  email,
  size = "lg",
}: {
  name: string;
  email: string;
  size?: "sm" | "lg";
}) {
  const letter = (name || email || "?").charAt(0).toUpperCase();

  // Deterministic color from the first character
  const colors = [
    "bg-blue-500",
    "bg-emerald-500",
    "bg-violet-500",
    "bg-amber-500",
    "bg-rose-500",
    "bg-cyan-500",
    "bg-pink-500",
    "bg-teal-500",
    "bg-orange-500",
    "bg-indigo-500",
  ];
  const colorIndex = (name || email).charCodeAt(0) % colors.length;

  return (
    <div
      className={`flex shrink-0 items-center justify-center rounded-full text-white font-semibold ${colors[colorIndex]} ${
        size === "lg" ? "size-14 text-xl" : "size-9 text-sm"
      }`}
    >
      {letter}
    </div>
  );
}

export { InitialsAvatar };

interface ContactCardProps {
  contact: Contact;
  onDelete: (id: string) => void;
  isDeleting: boolean;
  groups?: ContactGroup[];
  activeGroupId?: string | null;
}

interface ContactGroupsResponse {
  groups: ContactGroup[];
}

export function ContactCard({
  contact,
  onDelete,
  isDeleting,
  groups = [],
  activeGroupId,
}: ContactCardProps) {
  const addMember = useAddGroupMember();
  const removeMember = useRemoveGroupMember();

  // Fetch which groups this contact belongs to
  const { data: contactGroupsData } = useQuery({
    queryKey: ["contact-groups-for", contact.id],
    queryFn: async () => {
      // We don't have a direct endpoint, so derive from the groups list
      // by checking membership. For efficiency, we use the group members endpoint.
      // But since that's per-group, let's just use a simple approach:
      // check all groups' member lists. This is fine for small group counts.
      const membershipResults = await Promise.all(
        groups.map(async (g) => {
          try {
            const res = await apiGet<{ members: Contact[] }>(
              `/contact-groups/${g.id}/members`,
            );
            const isMember = res.members.some((m) => m.id === contact.id);
            return { groupId: g.id, isMember };
          } catch {
            return { groupId: g.id, isMember: false };
          }
        }),
      );
      return membershipResults;
    },
    enabled: groups.length > 0,
  });

  const memberGroupIds = new Set(
    contactGroupsData
      ?.filter((r) => r.isMember)
      .map((r) => r.groupId) ?? [],
  );

  const memberGroups = groups.filter((g) => memberGroupIds.has(g.id));
  const nonMemberGroups = groups.filter((g) => !memberGroupIds.has(g.id));

  return (
    <div className="rounded-lg border border-border bg-card p-6">
      <div className="flex items-start gap-4">
        <InitialsAvatar name={contact.name} email={contact.email} size="lg" />

        <div className="min-w-0 flex-1">
          <div className="flex items-start justify-between gap-2">
            <div>
              <h2 className="text-lg font-semibold text-foreground">
                {contact.name || contact.email}
              </h2>
              {contact.name && (
                <div className="mt-0.5 flex items-center gap-1.5 text-sm text-muted-foreground">
                  <Mail className="size-3.5" />
                  {contact.email}
                </div>
              )}
            </div>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => onDelete(contact.id)}
              disabled={isDeleting}
              className="text-muted-foreground hover:text-destructive"
              title="Delete contact"
            >
              <Trash2 className="size-4" />
            </Button>
          </div>

          <div className="mt-4 space-y-2">
            {contact.company && (
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <Building2 className="size-4 shrink-0" />
                <span>{contact.company}</span>
              </div>
            )}

            {contact.notes && (
              <div className="flex items-start gap-2 text-sm text-muted-foreground">
                <StickyNote className="size-4 shrink-0 mt-0.5" />
                <span>{contact.notes}</span>
              </div>
            )}

            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Tag className="size-4 shrink-0" />
              <span className="capitalize">{contact.source}</span>
            </div>

            {/* Groups section */}
            {groups.length > 0 && (
              <div className="flex items-start gap-2 text-sm text-muted-foreground">
                <Users className="size-4 shrink-0 mt-0.5" />
                <div className="flex flex-wrap gap-1">
                  {memberGroups.map((g) => (
                    <span
                      key={g.id}
                      className="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary"
                    >
                      {g.name}
                      <button
                        type="button"
                        onClick={() =>
                          removeMember.mutate({
                            groupId: g.id,
                            contactId: contact.id,
                          })
                        }
                        className="rounded-full p-0.5 hover:bg-primary/20"
                        title={`Remove from ${g.name}`}
                      >
                        <X className="size-3" />
                      </button>
                    </span>
                  ))}
                  {nonMemberGroups.length > 0 && (
                    <div className="relative group">
                      <button
                        type="button"
                        className="inline-flex items-center gap-1 rounded-full border border-dashed border-muted-foreground/30 px-2 py-0.5 text-xs text-muted-foreground hover:border-primary hover:text-primary"
                      >
                        <Plus className="size-3" />
                        Add to group
                      </button>
                      <div className="absolute left-0 top-full z-10 mt-1 hidden min-w-[140px] rounded-md border border-border bg-popover p-1 shadow-md group-hover:block">
                        {nonMemberGroups.map((g) => (
                          <button
                            key={g.id}
                            type="button"
                            onClick={() =>
                              addMember.mutate({
                                groupId: g.id,
                                contactId: contact.id,
                              })
                            }
                            className="flex w-full items-center gap-2 rounded-sm px-2 py-1 text-xs text-foreground hover:bg-accent"
                          >
                            {g.name}
                          </button>
                        ))}
                      </div>
                    </div>
                  )}
                  {memberGroups.length === 0 && nonMemberGroups.length === 0 && (
                    <span className="text-xs text-muted-foreground/60">
                      No groups
                    </span>
                  )}
                </div>
              </div>
            )}

            <div className="flex items-center gap-4 text-xs text-muted-foreground/70">
              <div className="flex items-center gap-1">
                <Clock className="size-3" />
                <span>Created {formatDate(contact.created_at)}</span>
              </div>
              {contact.last_contacted && (
                <span>Last contacted {formatDate(contact.last_contacted)}</span>
              )}
              {contact.contact_count > 0 && (
                <span>{contact.contact_count} interactions</span>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
