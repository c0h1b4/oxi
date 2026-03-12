"use client";

import { useAuthStore } from "@/stores/useAuthStore";
import { cn } from "@/lib/utils";
import { Plus } from "lucide-react";
import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { AddAccountModal } from "./AddAccountModal";

export function AccountSwitcher() {
  const accounts = useAuthStore((s) => s.accounts);
  const activeAccountId = useAuthStore((s) => s.activeAccountId);
  const setActiveAccount = useAuthStore((s) => s.setActiveAccount);
  const queryClient = useQueryClient();
  const [showAddModal, setShowAddModal] = useState(false);

  if (accounts.length === 0) {
    return null;
  }

  const handleSwitchAccount = (accountId: string) => {
    if (accountId === activeAccountId) return;
    setActiveAccount(accountId);
    queryClient.clear();
  };

  return (
    <div className="space-y-1">
      {accounts.map((account) => (
        <button
          key={account.id}
          type="button"
          onClick={() => handleSwitchAccount(account.id)}
          className={cn(
            "w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-left text-sm transition-colors",
            account.id === activeAccountId
              ? "bg-primary/10 text-primary"
              : "hover:bg-muted/50 text-muted-foreground"
          )}
        >
          <div className="size-6 rounded-full bg-muted flex items-center justify-center text-xs font-medium">
            {account.email[0].toUpperCase()}
          </div>
          <div className="flex-1 min-w-0">
            <div className="truncate">{account.email}</div>
            <div className="text-xs text-muted-foreground truncate">
              {account.imapHost}
            </div>
          </div>
        </button>
      ))}
      <button
        type="button"
        onClick={() => setShowAddModal(true)}
        className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-left text-sm text-muted-foreground hover:bg-muted/50 transition-colors"
      >
        <Plus className="size-4" />
        <span>Add account</span>
      </button>
      <AddAccountModal open={showAddModal} onClose={() => setShowAddModal(false)} />
    </div>
  );
}
