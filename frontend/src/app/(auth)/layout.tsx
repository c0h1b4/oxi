"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { fetchAccounts } from "@/lib/api";
import { useAuthStore } from "@/stores/useAuthStore";
import { ComposeDialog } from "@/components/mail/ComposeDialog";

export default function AuthLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const router = useRouter();
  const setAccounts = useAuthStore((s) => s.setAccounts);
  const existingAccounts = useAuthStore((s) => s.accounts);
  const [authenticated, setAuthenticated] = useState(existingAccounts.length > 0);

  useEffect(() => {
    if (authenticated) return;

    let cancelled = false;
    fetchAccounts()
      .then((data) => {
        if (!cancelled) {
          if (data.accounts.length > 0) {
            setAccounts(data.accounts);
            setAuthenticated(true);
          } else {
            router.replace("/");
          }
        }
      })
      .catch(() => {
        if (!cancelled) router.replace("/");
      });
    return () => {
      cancelled = true;
    };
  }, [router, setAccounts, authenticated]);

  if (!authenticated) {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <div className="flex flex-col items-center gap-3">
          <div className="size-8 animate-spin rounded-full border-4 border-muted border-t-primary" />
          <p className="text-sm text-muted-foreground">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <>
      {children}
      <ComposeDialog />
    </>
  );
}
