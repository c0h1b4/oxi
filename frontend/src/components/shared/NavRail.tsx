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
  MoreHorizontal,
} from "lucide-react";
import { DropdownMenu, Tooltip } from "radix-ui";
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
  const tooltipLabel = disabled ? `${label} (coming soon)` : label;

  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <button
          type="button"
          onClick={onClick}
          aria-label={tooltipLabel}
          className={cn(
            "flex size-11 shrink-0 md:size-10 items-center justify-center rounded-lg transition-colors",
            disabled
              ? "cursor-default text-sidebar-foreground/30"
              : "text-sidebar-foreground/70 hover:bg-sidebar-foreground/10 hover:text-sidebar-foreground",
            active && "bg-sidebar-accent text-sidebar-foreground",
          )}
        >
          {icon}
        </button>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content
          side="right"
          sideOffset={8}
          className="rounded-md bg-popover px-2.5 py-1.5 text-xs font-medium text-popover-foreground shadow-md border border-border animate-in fade-in-0 zoom-in-95"
        >
          {tooltipLabel}
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
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
    router.replace("/login");
  }, [router]);

  return (
    <Tooltip.Provider delayDuration={400}>
      <nav aria-label="Main navigation" className="relative flex w-full shrink-0 items-center justify-around border-b border-border bg-sidebar px-2 py-1 md:h-full md:w-14 md:flex-col md:justify-start md:overflow-y-auto md:border-b-0 md:border-r md:px-0 md:py-3">
        {/* Logo */}
        <div className="mb-4 hidden size-10 shrink-0 items-center justify-center md:flex">
          <span className="text-lg font-bold text-primary">o.</span>
        </div>

        {/* Top actions */}
        <div className="flex items-center gap-1 md:flex-col">
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
        <div className="hidden flex-1 md:block" />

        <div className="hidden md:block"><ConnectionStatus status={wsStatus} failCount={wsFailCount} /></div>

        {/* Bottom actions */}
        <div className="hidden flex-col items-center gap-1 md:flex">
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

        <DropdownMenu.Root>
          <DropdownMenu.Trigger asChild>
            <button type="button" aria-label="More options" className="flex size-11 shrink-0 items-center justify-center rounded-lg text-sidebar-foreground/70 hover:bg-sidebar-accent md:hidden">
              <MoreHorizontal className="size-5" />
            </button>
          </DropdownMenu.Trigger>
          <DropdownMenu.Portal>
            <DropdownMenu.Content align="end" sideOffset={4} className="z-50 min-w-48 rounded-lg border border-border bg-popover p-1 shadow-lg">
              <DropdownMenu.Item onClick={toggleTheme} className="flex min-h-11 cursor-pointer items-center gap-2 rounded px-3 text-sm outline-none focus:bg-accent">
                {resolvedTheme === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
                {resolvedTheme === "dark" ? "Light mode" : "Dark mode"}
              </DropdownMenu.Item>
              <DropdownMenu.Item onSelect={() => useUiStore.getState().setShortcutsOpen(true)} className="flex min-h-11 cursor-pointer items-center gap-2 rounded px-3 text-sm outline-none focus:bg-accent">
                <Keyboard className="size-4" /> Keyboard shortcuts
              </DropdownMenu.Item>
              <DropdownMenu.Item onSelect={handleLogout} className="flex min-h-11 cursor-pointer items-center gap-2 rounded px-3 text-sm outline-none focus:bg-accent">
                <LogOut className="size-4" /> Logout
              </DropdownMenu.Item>
              <ConnectionStatus status={wsStatus} failCount={wsFailCount} />
            </DropdownMenu.Content>
          </DropdownMenu.Portal>
        </DropdownMenu.Root>
      </nav>
    </Tooltip.Provider>
  );
}
