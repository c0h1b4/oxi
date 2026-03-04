"use client";

import { useCallback, useState } from "react";
import { useRouter } from "next/navigation";
import {
  Mail,
  PenSquare,
  Users,
  Settings,
  Moon,
  Sun,
  Keyboard,
  LogOut,
} from "lucide-react";
import { apiPost } from "@/lib/api";
import { cn } from "@/lib/utils";
import { useComposeStore } from "@/stores/useComposeStore";
import { useUiStore } from "@/stores/useUiStore";
import { ConnectionStatus } from "@/components/shared/ConnectionStatus";
import { useWsStatus } from "@/lib/ws-context";

function NavButton({
  icon,
  label,
  active,
  disabled,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}) {
  return (
    <button
      onClick={onClick}
      title={disabled ? `${label} (coming soon)` : label}
      className={cn(
        "flex size-10 items-center justify-center rounded-lg transition-colors",
        disabled
          ? "cursor-default text-sidebar-foreground/30"
          : "text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-foreground",
        active && "bg-sidebar-accent text-sidebar-foreground",
      )}
    >
      {icon}
    </button>
  );
}

export function NavRail() {
  const { status: wsStatus, failCount: wsFailCount } = useWsStatus();
  const router = useRouter();
  const viewMode = useUiStore((s) => s.viewMode);
  const setViewMode = useUiStore((s) => s.setViewMode);
  const [dark, setDark] = useState(() => {
    if (typeof window === "undefined") return false;
    const stored = localStorage.getItem("oxi-theme");
    const prefersDark =
      stored === "dark" ||
      (!stored && window.matchMedia("(prefers-color-scheme: dark)").matches);
    if (prefersDark) {
      document.documentElement.classList.add("dark");
    }
    return prefersDark;
  });
  const toggleDark = useCallback(() => {
    const next = !dark;
    setDark(next);
    if (next) {
      document.documentElement.classList.add("dark");
      localStorage.setItem("oxi-theme", "dark");
    } else {
      document.documentElement.classList.remove("dark");
      localStorage.setItem("oxi-theme", "light");
    }
  }, [dark]);

  const handleLogout = useCallback(async () => {
    try {
      await apiPost("/auth/logout", {});
    } catch {
      // Even if the API call fails, redirect to login
    }
    router.replace("/");
  }, [router]);

  return (
    <div className="relative flex h-full w-14 flex-col items-center border-r border-border bg-sidebar py-3">
      {/* Logo */}
      <div className="mb-4 flex size-10 items-center justify-center">
        <span className="text-lg font-bold text-primary">o.</span>
      </div>

      {/* Top actions */}
      <div className="flex flex-col items-center gap-1">
        <NavButton
          icon={<PenSquare className="size-5" />}
          label="Compose"
          onClick={() => useComposeStore.getState().openCompose()}
        />
        <NavButton
          icon={<Mail className="size-5" />}
          label="Mail"
          active={viewMode === "mail"}
          onClick={() => setViewMode("mail")}
        />
        <NavButton
          icon={<Users className="size-5" />}
          label="Contacts"
          active={viewMode === "contacts"}
          onClick={() => setViewMode(viewMode === "contacts" ? "mail" : "contacts")}
        />
        <NavButton
          icon={<Settings className="size-5" />}
          label="Settings"
          active={viewMode === "settings"}
          onClick={() => setViewMode(viewMode === "settings" ? "mail" : "settings")}
        />
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      <ConnectionStatus status={wsStatus} failCount={wsFailCount} />

      {/* Bottom actions */}
      <div className="flex flex-col items-center gap-1">
        <NavButton
          icon={
            dark ? (
              <Sun className="size-5" />
            ) : (
              <Moon className="size-5" />
            )
          }
          label={dark ? "Light mode" : "Dark mode"}
          onClick={toggleDark}
        />
        <NavButton
          icon={<Keyboard className="size-5" />}
          label="Keyboard shortcuts"
          onClick={() => useUiStore.getState().setShortcutsOpen(true)}
        />
        <NavButton
          icon={<LogOut className="size-5" />}
          label="Logout"
          onClick={handleLogout}
        />
      </div>

    </div>
  );
}
