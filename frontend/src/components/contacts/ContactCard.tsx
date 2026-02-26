"use client";

import { Trash2, Building2, Mail, StickyNote, Clock, Tag } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { Contact } from "@/types/contact";

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
}

export function ContactCard({ contact, onDelete, isDeleting }: ContactCardProps) {
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
