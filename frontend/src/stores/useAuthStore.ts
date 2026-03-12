"use client";

import { create } from "zustand";

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
  activeAccountId: null,
  
  setAccounts: (accounts) => set((state) => { 
    const currentActiveExists = accounts.some((a) => a.id === state.activeAccountId);
    return {
      accounts, 
      activeAccountId: currentActiveExists 
        ? state.activeAccountId 
        : (accounts.length > 0 ? accounts[0].id : null),
    };
  }),
  
  addAccount: (account) => set((state) => ({ 
    accounts: [...state.accounts, account],
    activeAccountId: state.activeAccountId ?? account.id,
  })),
  
  removeAccount: (id) => set((state) => {
    const newAccounts = state.accounts.filter((a) => a.id !== id);
    return {
      accounts: newAccounts,
      activeAccountId: state.activeAccountId === id 
        ? (newAccounts[0]?.id ?? null)
        : state.activeAccountId,
    };
  }),
  
  setActiveAccount: (id) => set({ activeAccountId: id }),
  
  activeAccount: () => get().accounts.find((a) => a.id === get().activeAccountId),
}));
