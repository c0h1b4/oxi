"use client";

import { create } from "zustand";
import type { Account } from "@/stores/useAuthStore";

const STORAGE_PREFIX = "oxi-remote-resource-trust:";

/** Account IDs are session-specific; mailbox + IMAP host survive re-login. */
export function remoteResourceAccountScope(account: Account | undefined): string | null {
  if (!account?.email.trim() || !account.imapHost.trim()) return null;
  return JSON.stringify([account.imapHost.trim().toLowerCase(), account.email.trim()]);
}

/** Match a single mailbox, never a display name, domain, or address list. */
export function normalizeSenderAddress(address: string): string | null {
  const mailbox = address.trim();
  if (!/^[^\s@<>,;]+@[^\s@<>,;]+$/.test(mailbox)) return null;
  const at = mailbox.lastIndexOf("@");
  // Local parts can be case-sensitive. Only domain names are case-insensitive.
  return mailbox.slice(0, at) + mailbox.slice(at).toLowerCase();
}

function loadSenders(scope: string): string[] {
  if (typeof window === "undefined") return [];
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(STORAGE_PREFIX + scope) ?? "[]");
    if (!Array.isArray(parsed)) return [];
    return [...new Set(parsed.flatMap((value: unknown) => {
      const sender = typeof value === "string" ? normalizeSenderAddress(value) : null;
      return sender ? [sender] : [];
    }))];
  } catch {
    return [];
  }
}

interface RemoteResourceState {
  sendersByAccount: Record<string, string[]>;
  loadAccount: (scope: string) => void;
  trustSender: (scope: string, address: string) => void;
}

export const useRemoteResourceStore = create<RemoteResourceState>((set) => ({
  sendersByAccount: {},
  loadAccount: (scope) => set((state) => ({
    sendersByAccount: { ...state.sendersByAccount, [scope]: loadSenders(scope) },
  })),
  trustSender: (scope, address) => {
    const sender = normalizeSenderAddress(address);
    if (!scope || !sender) return;
    set((state) => {
      const senders = [...new Set([
        ...loadSenders(scope),
        ...(state.sendersByAccount[scope] ?? []),
        sender,
      ])];
      try {
        localStorage.setItem(STORAGE_PREFIX + scope, JSON.stringify(senders));
      } catch {
        // Keep the user's choice in memory if browser storage is unavailable.
      }
      return { sendersByAccount: { ...state.sendersByAccount, [scope]: senders } };
    });
  },
}));
