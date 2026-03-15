"use client";

import { useState, useCallback } from "react";
import { Popover, Dialog } from "radix-ui";
import { AnimatePresence, motion } from "framer-motion";
import { Send, Copy, Check, UserPlus, X, Search } from "lucide-react";
import { useComposeStore } from "@/stores/useComposeStore";
import { useCreateContact } from "@/hooks/useContacts";
import { createFadeSlideVariants, createScaleFadeVariants } from "@/lib/motion/variants";
import { useUiStore } from "@/stores/useUiStore";
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
        <button type="button" className="inline rounded px-0.5 text-sm text-foreground underline decoration-muted-foreground/30 underline-offset-2 hover:bg-accent hover:decoration-foreground">
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
            type="button"
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
            type="button"
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
            type="button"
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

function HighlightedText({
  text,
  query,
}: {
  text: string;
  query: string;
}) {
  if (!query) return <>{text}</>;

  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const index = lowerText.indexOf(lowerQuery);

  if (index === -1) return <>{text}</>;

  const before = text.slice(0, index);
  const match = text.slice(index, index + query.length);
  const after = text.slice(index + query.length);

  return (
    <>
      {before}
      <mark className="bg-yellow-200 dark:bg-yellow-800/50 text-inherit rounded px-0.5">
        {match}
      </mark>
      {after}
    </>
  );
}

function HighlightedRecipient({
  address,
  name,
  query,
}: {
  address: string;
  name?: string | null;
  query: string;
}) {
  const createContact = useCreateContact();
  const [contactAdded, setContactAdded] = useState(false);

  return (
    <Popover.Root
      onOpenChange={(open) => {
        if (!open) setContactAdded(false);
      }}
    >
      <Popover.Trigger asChild>
        <button
          type="button"
          className="inline rounded px-0.5 text-sm text-foreground underline decoration-muted-foreground/30 underline-offset-2 hover:bg-accent hover:decoration-foreground text-left"
        >
          {name ? (
            <span>
              <HighlightedText text={name} query={query} />
              {" <"}
              <span className="text-muted-foreground">
                <HighlightedText text={address} query={query} />
              </span>
              {">"}
            </span>
          ) : (
            <HighlightedText text={address} query={query} />
          )}
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
            type="button"
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
            type="button"
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
            type="button"
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

function RecipientModal({
  addresses,
  open,
  onOpenChange,
}: {
  addresses: EmailAddress[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const shouldAnimate = effectiveAnimationMode !== "off";
  const overlayMotionProps = createFadeSlideVariants(effectiveAnimationMode, "y");
  const contentMotionProps = createScaleFadeVariants(effectiveAnimationMode);
  const ContentContainer = shouldAnimate ? motion.div : "div";
  const [filter, setFilter] = useState("");

  const handleEscapeKeyDown = useCallback(
    (e: Event) => {
      e.preventDefault();
      e.stopPropagation();
      if (filter) {
        setFilter("");
      } else {
        onOpenChange(false);
      }
    },
    [filter, onOpenChange],
  );

  const filteredAddresses = addresses.filter((a) => {
    if (!filter) return true;
    const query = filter.toLowerCase();
    return (
      a.address.toLowerCase().includes(query) ||
      (a.name && a.name.toLowerCase().includes(query))
    );
  });

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <AnimatePresence>
          {open ? (
            <>
              <Dialog.Overlay asChild={shouldAnimate}>
                {shouldAnimate ? (
                  <motion.div
                    data-testid="recipient-modal-overlay-transition"
                    data-motion-props={JSON.stringify(overlayMotionProps)}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    variants={overlayMotionProps}
                    className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm"
                  />
                ) : (
                  <div className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm" />
                )}
              </Dialog.Overlay>
              <Dialog.Content
                asChild={shouldAnimate}
                onEscapeKeyDown={handleEscapeKeyDown}
                className={
                  shouldAnimate
                    ? undefined
                    : "fixed left-[50%] top-[50%] z-50 grid w-full max-w-lg translate-x-[-50%] translate-y-[-50%] gap-4 border bg-background p-6 shadow-lg sm:rounded-lg"
                }
              >
                <ContentContainer
                  {...(shouldAnimate
                    ? {
                        "data-testid": "recipient-modal-content-transition",
                        "data-motion-props": JSON.stringify(contentMotionProps),
                        initial: "initial",
                        animate: "animate",
                        exit: "exit",
                        variants: contentMotionProps,
                        className:
                          "fixed left-[50%] top-[50%] z-50 grid w-full max-w-lg translate-x-[-50%] translate-y-[-50%] gap-4 border bg-background p-6 shadow-lg sm:rounded-lg",
                      }
                    : {})}
                >
                  <div className="flex flex-col space-y-1.5 text-center sm:text-left">
                    <Dialog.Title className="text-lg font-semibold leading-none tracking-tight">
                      {addresses.length} Recipients
                    </Dialog.Title>
                    <Dialog.Description className="text-sm text-muted-foreground">
                      All recipients for this email.
                    </Dialog.Description>
                  </div>

                  <div className="relative">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                    <input
                      type="text"
                      placeholder="Find recipient..."
                      value={filter}
                      onChange={(e) => setFilter(e.target.value)}
                      className="w-full rounded-md border border-input bg-background pl-9 pr-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                    />
                    {filter && (
                      <button
                        type="button"
                        onClick={() => setFilter("")}
                        className="absolute right-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground hover:text-foreground"
                      >
                        <X className="h-4 w-4" />
                      </button>
                    )}
                  </div>

                  <div className="max-h-[50vh] overflow-y-auto pr-2 flex flex-col gap-2">
                    {filteredAddresses.length === 0 ? (
                      <p className="text-sm text-muted-foreground text-center py-4">
                        No recipients match your search.
                      </p>
                    ) : (
                      filteredAddresses.map((a, i) => (
                        <div
                          key={`${a.address}-${i}`}
                          className="flex flex-col text-sm border-b border-border/40 pb-2 last:border-0 last:pb-0"
                        >
                          <div className="font-medium text-foreground">
                            <HighlightedRecipient
                              address={a.address}
                              name={a.name}
                              query={filter}
                            />
                          </div>
                        </div>
                      ))
                    )}
                  </div>

                  {filter && (
                    <p className="text-xs text-muted-foreground text-center">
                      Showing {filteredAddresses.length} of {addresses.length} recipients
                    </p>
                  )}

                  <Dialog.Close asChild>
                    <button
                      type="button"
                      className="absolute right-4 top-4 rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:pointer-events-none"
                    >
                      <X className="h-4 w-4" />
                      <span className="sr-only">Close</span>
                    </button>
                  </Dialog.Close>
                </ContentContainer>
              </Dialog.Content>
            </>
          ) : null}
        </AnimatePresence>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function AddressList({ addresses }: { addresses: EmailAddress[] }) {
  const [modalOpen, setModalOpen] = useState(false);
  const [modalKey, setModalKey] = useState(0);

  if (addresses.length <= 5) {
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

  const firstFive = addresses.slice(0, 5);
  const remainingCount = addresses.length - 5;

  const handleOpenModal = () => {
    setModalKey((k) => k + 1);
    setModalOpen(true);
  };

  return (
    <>
      <span className="inline">
        {firstFive.map((a, i) => (
          <span key={`${a.address}-${i}`}>
            {i > 0 && ", "}
            <AddressChip address={a.address} name={a.name} />
          </span>
        ))}
        {" "}
        <button
          type="button"
          onClick={handleOpenModal}
          className="inline rounded px-1.5 py-[1px] text-[11px] font-medium bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors cursor-pointer border border-border/40 select-none align-middle ml-1 relative -top-[1px]"
        >
          + {remainingCount} others
        </button>
      </span>

      <RecipientModal
        key={modalKey}
        addresses={addresses}
        open={modalOpen}
        onOpenChange={setModalOpen}
      />
    </>
  );
}
