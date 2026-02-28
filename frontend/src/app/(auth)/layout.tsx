"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { apiGet } from "@/lib/api";
import { useAuthStore } from "@/stores/useAuthStore";

export default function AuthLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const router = useRouter();
  const [authenticated, setAuthenticated] = useState(false);
  const setEmail = useAuthStore((s) => s.setEmail);

  useEffect(() => {
    let cancelled = false;
    apiGet<{ user: { email: string } }>("/auth/session")
      .then((data) => {
        if (!cancelled) {
          setEmail(data.user.email);
          setAuthenticated(true);
        }
      })
      .catch(() => {
        if (!cancelled) router.replace("/");
      });
    return () => {
      cancelled = true;
    };
  }, [router, setEmail]);

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

  return <>{children}</>;
}
