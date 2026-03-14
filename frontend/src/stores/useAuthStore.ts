"use client";

import { create } from "zustand";

const ACTIVE_ACCOUNT_STORAGE_KEY = "oxi-active-account-id";

function loadActiveAccountId(): string | null {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    return localStorage.getItem(ACTIVE_ACCOUNT_STORAGE_KEY);
  } catch {
    return null;
  }
}

function saveActiveAccountId(id: string | null) {
  if (typeof window === "undefined") {
    return;
  }

  try {
    if (id) {
      localStorage.setItem(ACTIVE_ACCOUNT_STORAGE_KEY, id);
      return;
    }

    localStorage.removeItem(ACTIVE_ACCOUNT_STORAGE_KEY);
  } catch {
  }
}

export interface Account {
  id: string;
  email: string;
  imapHost: string;
  smtpHost: string;
}

interface AuthState {
  accounts: Account[];
  activeAccountId: string | null;
  
  setAccounts: (accounts: Account[]) => void;
  addAccount: (account: Account) => void;
  removeAccount: (id: string) => void;
  setActiveAccount: (id: string) => void;
  activeAccount: () => Account | undefined;
}

export const useAuthStore = create<AuthState>((set, get) => ({
  accounts: [],
  activeAccountId: loadActiveAccountId(),
  
  setAccounts: (accounts) => set((state) => { 
    const currentActiveExists = accounts.some((a) => a.id === state.activeAccountId);
    const nextActiveAccountId = currentActiveExists
      ? state.activeAccountId
      : (accounts.length > 0 ? accounts[0].id : null);

    saveActiveAccountId(nextActiveAccountId);

    return {
      accounts, 
      activeAccountId: nextActiveAccountId,
    };
  }),
  
  addAccount: (account) => set((state) => {
    const nextActiveAccountId = state.activeAccountId ?? account.id;
    saveActiveAccountId(nextActiveAccountId);

    return {
      accounts: [...state.accounts, account],
      activeAccountId: nextActiveAccountId,
    };
  }),
  
  removeAccount: (id) => set((state) => {
    const newAccounts = state.accounts.filter((a) => a.id !== id);
    const nextActiveAccountId = state.activeAccountId === id
      ? (newAccounts[0]?.id ?? null)
      : state.activeAccountId;

    saveActiveAccountId(nextActiveAccountId);

    return {
      accounts: newAccounts,
      activeAccountId: nextActiveAccountId,
    };
  }),
  
  setActiveAccount: (id) => {
    saveActiveAccountId(id);
    set({ activeAccountId: id });
  },
  
  activeAccount: () => get().accounts.find((a) => a.id === get().activeAccountId),
}));
