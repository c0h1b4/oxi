"use client";

import { useState } from "react";
import { Popover } from "radix-ui";
import { Send, Copy, Check, UserPlus } from "lucide-react";
import { useComposeStore } from "@/stores/useComposeStore";
import { useCreateContact } from "@/hooks/useContacts";
import type { EmailAddress } from "@/types/message";

export function AddressChip({
  address,
  name,
}: {
  address: string;
  name?: string | null;
}) {
  const displayName = name || address;
  const createContact = useCreateContact();
  const [contactAdded, setContactAdded] = useState(false);

  return (
    <Popover.Root
      onOpenChange={(open) => {
        if (!open) setContactAdded(false);
      }}
    >
      <Popover.Trigger asChild>
        <button className="inline rounded px-0.5 text-sm text-foreground underline decoration-muted-foreground/30 underline-offset-2 hover:bg-accent hover:decoration-foreground">
          {displayName}
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          className="z-50 w-56 rounded-lg border border-border bg-background p-1 shadow-lg"
          sideOffset={4}
          align="start"
        >
          <div className="border-b border-border px-3 py-2">
            {name && (
              <p className="text-sm font-medium truncate">{name}</p>
            )}
            <p className="text-xs text-muted-foreground truncate">
              {address}
            </p>
          </div>
          <button
            onClick={() => {
              useComposeStore.getState().openCompose();
              useComposeStore.setState({ to: address });
            }}
            className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm hover:bg-accent"
          >
            <Send className="size-3.5 text-muted-foreground" />
            Compose email to
          </button>
          <button
            onClick={() => {
              navigator.clipboard.writeText(
                name ? `${name} <${address}>` : address,
              );
            }}
            className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm hover:bg-accent"
          >
            <Copy className="size-3.5 text-muted-foreground" />
            Copy address
          </button>
          <button
            disabled={contactAdded || createContact.isPending}
            onClick={() => {
              createContact.mutate(
                { email: address, name: name ?? "" },
                { onSuccess: () => setContactAdded(true) },
              );
            }}
            className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm hover:bg-accent disabled:opacity-50"
          >
            {contactAdded ? (
              <>
                <Check className="size-3.5 text-green-500" />
                Contact added
              </>
            ) : (
              <>
                <UserPlus className="size-3.5 text-muted-foreground" />
                Add to contacts
              </>
            )}
          </button>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

export function AddressList({ addresses }: { addresses: EmailAddress[] }) {
  return (
    <span className="inline">
      {addresses.map((a, i) => (
        <span key={`${a.address}-${i}`}>
          {i > 0 && ", "}
          <AddressChip address={a.address} name={a.name} />
        </span>
      ))}
    </span>
  );
}
