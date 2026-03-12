"use client";

import { useAuthStore } from "@/stores/useAuthStore";
import { cn } from "@/lib/utils";
import { Plus, ChevronDown, LogOut, X } from "lucide-react";
import { useState, useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useFolders } from "@/hooks/useFolders";
import { apiDelete, apiPost } from "@/lib/api";
import { useRouter } from "next/navigation";
import { AddAccountModal } from "./AddAccountModal";

export function AccountSwitcher() {
  const accounts = useAuthStore((s) => s.accounts);
  const activeAccountId = useAuthStore((s) => s.activeAccountId);
  const setActiveAccount = useAuthStore((s) => s.setActiveAccount);
  const removeAccount = useAuthStore((s) => s.removeAccount);
  const setAccounts = useAuthStore((s) => s.setAccounts);
  const queryClient = useQueryClient();
  const router = useRouter();
  const [showAddModal, setShowAddModal] = useState(false);
  const [isOpen, setIsOpen] = useState(false);
  const [loggingOut, setLoggingOut] = useState<string | null>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const { data: foldersData } = useFolders();
  const inboxUnread = foldersData?.folders.find((f) => f.name === "INBOX")?.unread_count ?? 0;

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  if (accounts.length === 0) {
    return null;
  }

  const activeAccount = accounts.find((a) => a.id === activeAccountId);

  const handleSwitchAccount = (accountId: string) => {
    if (accountId === activeAccountId) return;
    setActiveAccount(accountId);
    queryClient.clear();
    setIsOpen(false);
  };

  const handleLogoutAccount = async (accountId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setLoggingOut(accountId);
    try {
      await apiDelete(`/auth/accounts/${accountId}`);
      removeAccount(accountId);
      if (accountId === activeAccountId) {
        queryClient.clear();
      }
    } catch (err) {
      console.error("Failed to logout account:", err);
    } finally {
      setLoggingOut(null);
    }
  };

  const handleLogoutAll = async () => {
    setLoggingOut("all");
    try {
      await apiPost("/auth/logout", {});
      setAccounts([]);
      queryClient.clear();
      router.push("/");
    } catch (err) {
      console.error("Failed to logout all accounts:", err);
    } finally {
      setLoggingOut(null);
    }
  };

  return (
    <div ref={dropdownRef} className="relative">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={cn(
          "w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-left text-sm transition-colors",
          "hover:bg-muted/50",
          isOpen && "bg-muted/50"
        )}
      >
        <div className="size-7 rounded-full bg-primary/10 text-primary flex items-center justify-center text-xs font-medium shrink-0">
          {activeAccount?.email[0]?.toUpperCase() ?? "?"}
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="truncate font-medium">{activeAccount?.email}</span>
            {inboxUnread > 0 && (
              <span className="shrink-0 rounded-full bg-primary px-1.5 py-0.5 text-[10px] font-medium text-primary-foreground">
                {inboxUnread}
              </span>
            )}
          </div>
        </div>
        <ChevronDown
          className={cn(
            "size-4 text-muted-foreground shrink-0 transition-transform",
            isOpen && "rotate-180"
          )}
        />
      </button>

      {isOpen && (
        <div className="absolute top-0 left-full ml-2 w-64 bg-background border border-border rounded-md shadow-lg z-50 py-1">
          {accounts.map((account) => {
            const isActive = account.id === activeAccountId;
            const unread = isActive ? inboxUnread : null;
            const isLoggingOut = loggingOut === account.id;

            return (
              <button
                key={account.id}
                type="button"
                onClick={() => handleSwitchAccount(account.id)}
                className={cn(
                  "w-full flex items-center gap-2 px-2 py-1.5 text-left text-sm transition-colors",
                  isActive ? "bg-primary/5" : "hover:bg-muted/50"
                )}
              >
                <div className={cn(
                  "size-6 rounded-full flex items-center justify-center text-xs font-medium shrink-0",
                  isActive ? "bg-primary/10 text-primary" : "bg-muted text-muted-foreground"
                )}>
                  {account.email[0]?.toUpperCase() ?? "?"}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className={cn("truncate", !isActive && "text-muted-foreground")}>
                      {account.email}
                    </span>
                    {unread !== null && unread > 0 && (
                      <span className="shrink-0 rounded-full bg-primary px-1.5 py-0.5 text-[10px] font-medium text-primary-foreground">
                        {unread}
                      </span>
                    )}
                  </div>
                </div>
                <button
                  type="button"
                  onClick={(e) => handleLogoutAccount(account.id, e)}
                  disabled={isLoggingOut}
                  className={cn(
                    "shrink-0 p-1 rounded hover:bg-muted text-muted-foreground hover:text-foreground transition-colors",
                    isLoggingOut && "opacity-50 cursor-not-allowed"
                  )}
                  title="Sign out this account"
                >
                  <X className="size-3.5" />
                </button>
              </button>
            );
          })}

          <div className="border-t border-border mt-1 pt-1">
            <button
              type="button"
              onClick={() => {
                setIsOpen(false);
                setShowAddModal(true);
              }}
              className="w-full flex items-center gap-2 px-2 py-1.5 text-left text-sm text-muted-foreground hover:bg-muted/50 transition-colors"
            >
              <Plus className="size-4" />
              <span>Add account</span>
            </button>

            {accounts.length > 1 && (
              <button
                type="button"
                onClick={handleLogoutAll}
                disabled={loggingOut === "all"}
                className={cn(
                  "w-full flex items-center gap-2 px-2 py-1.5 text-left text-sm text-muted-foreground hover:bg-muted/50 transition-colors",
                  loggingOut === "all" && "opacity-50 cursor-not-allowed"
                )}
              >
                <LogOut className="size-4" />
                <span>Sign out all accounts</span>
              </button>
            )}
          </div>
        </div>
      )}

      <AddAccountModal open={showAddModal} onClose={() => setShowAddModal(false)} />
    </div>
  );
}
