"use client";

import { QueryClientProvider } from "@tanstack/react-query";
import { LazyMotion, domAnimation } from "framer-motion";
import { useEffect, useState } from "react";
import { Toaster } from "sonner";
import { MotionProvider } from "@/lib/motion/MotionProvider";
import { makeQueryClient } from "@/lib/query-client";

const THEME_STORAGE_KEY = "oxi-theme";

function clearThemeTransitionArtifacts() {
  document.querySelectorAll("[data-theme-transition]").forEach((node) => {
    node.remove();
  });
  document.documentElement.classList.remove("theme-transitioning");
  document.documentElement.classList.remove("disable-transitions");
  document.documentElement.style.removeProperty("--click-x");
  document.documentElement.style.removeProperty("--click-y");
}

function ThemeInitializer() {
  useEffect(() => {
    clearThemeTransitionArtifacts();
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    if (stored === "dark") {
      document.documentElement.classList.add("dark");
    } else if (stored === "light") {
      document.documentElement.classList.remove("dark");
    } else {
      const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      document.documentElement.classList.toggle("dark", prefersDark);
    }
  }, []);

  return null;
}

export function Providers({ children }: { children: React.ReactNode }) {
  const [queryClient] = useState(() => makeQueryClient());
  return (
    <QueryClientProvider client={queryClient}>
      <ThemeInitializer />
      <LazyMotion features={domAnimation}>
        <MotionProvider>{children}</MotionProvider>
      </LazyMotion>
      <Toaster
        position="bottom-right"
        toastOptions={{
          className:
            "!bg-foreground !text-background !border-border !shadow-lg !rounded-lg !text-sm",
        }}
      />
    </QueryClientProvider>
  );
}
