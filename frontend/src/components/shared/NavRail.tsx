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
import { Tooltip } from "radix-ui";
import { apiPost } from "@/lib/api";
import { useT } from "@/hooks/useI18n";
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
  const t = useT();
  const tooltipLabel = disabled ? t("nav.comingSoon", { label }) : label;

  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <button
          type="button"
          onClick={onClick}
          aria-label={tooltipLabel}
          className={cn(
            "flex size-10 items-center justify-center rounded-lg transition-colors",
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
  const t = useT();
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
    <Tooltip.Provider delayDuration={400}>
      <div className="relative flex h-full w-14 flex-col items-center border-r border-border bg-sidebar py-3">
        {/* Logo */}
        <div className="mb-4 flex size-10 items-center justify-center">
          <span className="text-lg font-bold text-primary">o.</span>
        </div>

        {/* Top actions */}
        <div className="flex flex-col items-center gap-1">
          <NavButton
            icon={<PenSquare className="size-5" />}
            label={t("nav.compose")}
            onClick={() => useComposeStore.getState().openCompose()}
          />
          <NavButton
            icon={<Mail className="size-5" />}
            label={t("nav.mail")}
            active={viewMode === "mail"}
            onClick={() => setViewMode("mail")}
          />
          <NavButton
            icon={<Users className="size-5" />}
            label={t("nav.contacts")}
            active={viewMode === "contacts"}
            onClick={() => setViewMode(viewMode === "contacts" ? "mail" : "contacts")}
          />
          <NavButton
            icon={<Calendar className="size-5" />}
            label={t("nav.calendar")}
            active={viewMode === "calendar"}
            onClick={() => setViewMode(viewMode === "calendar" ? "mail" : "calendar")}
          />
          <NavButton
            icon={<Settings className="size-5" />}
            label={t("nav.settings")}
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
            label={resolvedTheme === "dark" ? t("nav.lightMode") : t("nav.darkMode")}
            onClick={toggleTheme}
          />
          <NavButton
            icon={<Keyboard className="size-5" />}
            label={t("nav.shortcuts")}
            onClick={() => useUiStore.getState().setShortcutsOpen(true)}
          />
          <NavButton
            icon={<LogOut className="size-5" />}
            label={t("nav.logout")}
            onClick={handleLogout}
          />
        </div>

      </div>
    </Tooltip.Provider>
  );
}
