"use client";

import { useCallback, type MouseEvent } from "react";
import { useRouter } from "next/navigation";
import {
  Mail,
  PenSquare,
  Users,
  Calendar,
  Settings,
  Moon,
  Sun,
  Keyboard,
  LogOut,
} from "lucide-react";
import { apiPost } from "@/lib/api";
import { runThemeSpreadTransition } from "@/lib/motion/theme-spread";
import { cn } from "@/lib/utils";
import { useComposeStore } from "@/stores/useComposeStore";
import { useUiStore } from "@/stores/useUiStore";
import { useUpdateDisplayPreferences } from "@/hooks/useDisplayPreferences";
import { ConnectionStatus } from "@/components/shared/ConnectionStatus";
import { useWsStatus } from "@/lib/ws-context";

type NavButtonClickEvent = MouseEvent<Element>;

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
  onClick?: (event: NavButtonClickEvent) => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={disabled ? `${label} (coming soon)` : label}
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

function useResolvedTheme() {
  const theme = useUiStore((s) => s.theme);
  if (theme === "system") {
    if (typeof window === "undefined") return "light";
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return theme;
}

const THEME_STORAGE_KEY = "oxi-theme";

export function NavRail() {
  const { status: wsStatus, failCount: wsFailCount } = useWsStatus();
  const router = useRouter();
  const viewMode = useUiStore((s) => s.viewMode);
  const setViewMode = useUiStore((s) => s.setViewMode);
  const setTheme = useUiStore((s) => s.setTheme);
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const updatePrefs = useUpdateDisplayPreferences();
  const resolvedTheme = useResolvedTheme();

  const toggleTheme = useCallback((event: NavButtonClickEvent) => {
    const next = resolvedTheme === "dark" ? "light" : "dark";
    runThemeSpreadTransition({
      mode: effectiveAnimationMode,
      trigger: "explicit",
      origin: { x: event.clientX, y: event.clientY },
      applyTheme: () => setTheme(next),
      nextTheme: next,
    });
    localStorage.setItem(THEME_STORAGE_KEY, next);
    updatePrefs.mutate({ theme: next });
  }, [effectiveAnimationMode, resolvedTheme, setTheme, updatePrefs]);

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
          icon={<Calendar className="size-5" />}
          label="Calendar"
          active={viewMode === "calendar"}
          onClick={() => setViewMode(viewMode === "calendar" ? "mail" : "calendar")}
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
            resolvedTheme === "dark" ? (
              <Sun className="size-5" />
            ) : (
              <Moon className="size-5" />
            )
          }
          label={resolvedTheme === "dark" ? "Light mode" : "Dark mode"}
          onClick={toggleTheme}
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
