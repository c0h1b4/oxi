"use client";

import { useState, type FormEvent, useEffect, useRef } from "react";
import { Dialog } from "radix-ui";
import { X, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { apiPost, fetchAccounts } from "@/lib/api";
import { useAuthStore } from "@/stores/useAuthStore";
import { useQueryClient } from "@tanstack/react-query";

function getCookie(name: string): string | null {
  if (typeof document === "undefined") return null;
  const match = document.cookie.match(new RegExp(`(^| )${name}=([^;]+)`));
  return match ? match[2] : null;
}

interface ServerConfig {
  imapHost: string;
  imapPort: string;
  imapTls: boolean;
  smtpHost: string;
  smtpPort: string;
  smtpTls: boolean;
}

interface AddAccountModalProps {
  open: boolean;
  onClose: () => void;
}

export function AddAccountModal({ open, onClose }: AddAccountModalProps) {
  const setAccounts = useAuthStore((s) => s.setAccounts);
  const setActiveAccount = useAuthStore((s) => s.setActiveAccount);
  const queryClient = useQueryClient();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [remember, setRemember] = useState(false);
  const [loading, setLoading] = useState(false);
  const [showServerConfig, setShowServerConfig] = useState(false);
  const [serverConfig, setServerConfig] = useState<ServerConfig>({
    imapHost: "",
    imapPort: "993",
    imapTls: true,
    smtpHost: "",
    smtpPort: "587",
    smtpTls: true,
  });

  const emailInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setEmail("");
      setPassword("");
      setError(null);
      setRemember(false);
      setShowServerConfig(false);
      setServerConfig({
        imapHost: "",
        imapPort: "993",
        imapTls: true,
        smtpHost: "",
        smtpPort: "587",
        smtpTls: true,
      });
      const timer = setTimeout(() => emailInputRef.current?.focus(), 50);
      return () => clearTimeout(timer);
    }
  }, [open]);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setLoading(true);
    setError(null);

    const browserId = getCookie("oxi_browser");

    const payload: Record<string, unknown> = {
      email,
      password,
      remember,
      imap_tls: serverConfig.imapTls,
      smtp_tls: serverConfig.smtpTls,
    };

    if (browserId) {
      payload.browser_id = browserId;
    }

    if (serverConfig.imapHost) {
      payload.imap_host = serverConfig.imapHost;
    }
    if (serverConfig.imapPort) {
      const port = parseInt(serverConfig.imapPort, 10);
      if (!isNaN(port)) payload.imap_port = port;
    }
    if (serverConfig.smtpHost) {
      payload.smtp_host = serverConfig.smtpHost;
    }
    if (serverConfig.smtpPort) {
      const port = parseInt(serverConfig.smtpPort, 10);
      if (!isNaN(port)) payload.smtp_port = port;
    }

    try {
      const response = await apiPost<{
        account: { id: string; email: string; imapHost: string; smtpHost: string };
      }>("/auth/login", payload);
      
      const accountsData = await fetchAccounts();
      setAccounts(accountsData.accounts);
      setActiveAccount(response.account.id);
      queryClient.clear();
      onClose();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "An unexpected error occurred",
      );
    } finally {
      setLoading(false);
    }
  }

  return (
    <Dialog.Root open={open} onOpenChange={(o) => !o && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl border border-border bg-background p-6 shadow-2xl max-h-[90vh] overflow-y-auto">
          <Dialog.Title className="text-lg font-semibold mb-1">
            Add account
          </Dialog.Title>
          <Dialog.Description className="text-sm text-muted-foreground mb-4">
            Sign in to another email account
          </Dialog.Description>
          <Dialog.Close asChild>
            <button
              type="button"
              className="absolute right-4 top-4 rounded-md p-1 text-muted-foreground hover:text-foreground transition-colors"
              aria-label="Close"
            >
              <X className="size-4" />
            </button>
          </Dialog.Close>

          {error && (
            <div className="mb-4 rounded-md bg-destructive/10 px-4 py-3 text-sm text-destructive">
              {error}
            </div>
          )}

          <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="add-email">Email</Label>
              <Input
                ref={emailInputRef}
                id="add-email"
                type="email"
                placeholder="you@example.com"
                autoComplete="email"
                required
                disabled={loading}
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="add-password">Password</Label>
              <Input
                id="add-password"
                type="password"
                placeholder="Enter your password"
                autoComplete="current-password"
                required
                disabled={loading}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </div>
            <label htmlFor="add-remember" className="flex items-center gap-2 cursor-pointer select-none">
              <input
                id="add-remember"
                type="checkbox"
                checked={remember}
                onChange={(e) => setRemember(e.target.checked)}
                disabled={loading}
                className="size-4 rounded border-muted-foreground/40 accent-primary"
              />
              <span className="text-sm text-muted-foreground">Keep me logged in</span>
            </label>

            <button
              type="button"
              onClick={() => setShowServerConfig(!showServerConfig)}
              className="text-left text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              {showServerConfig ? "▼" : "▶"} Server configuration (optional)
            </button>

            {showServerConfig && (
              <div className="flex flex-col gap-3 rounded-md border border-border p-3">
                <div className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                  IMAP Settings
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <div className="flex flex-col gap-1">
                    <Label htmlFor="add-imapHost" className="text-xs">Host</Label>
                    <Input
                      id="add-imapHost"
                      type="text"
                      placeholder="imap.example.com"
                      disabled={loading}
                      value={serverConfig.imapHost}
                      onChange={(e) => setServerConfig({ ...serverConfig, imapHost: e.target.value })}
                      className="h-8 text-sm"
                    />
                  </div>
                  <div className="flex flex-col gap-1">
                    <Label htmlFor="add-imapPort" className="text-xs">Port</Label>
                    <Input
                      id="add-imapPort"
                      type="number"
                      placeholder="993"
                      disabled={loading}
                      value={serverConfig.imapPort}
                      onChange={(e) => setServerConfig({ ...serverConfig, imapPort: e.target.value })}
                      className="h-8 text-sm"
                    />
                  </div>
                </div>
                <label htmlFor="add-imapTls" className="flex items-center gap-2 cursor-pointer select-none">
                  <input
                    id="add-imapTls"
                    type="checkbox"
                    checked={serverConfig.imapTls}
                    onChange={(e) => setServerConfig({ ...serverConfig, imapTls: e.target.checked })}
                    disabled={loading}
                    className="size-3 rounded border-muted-foreground/40 accent-primary"
                  />
                  <span className="text-xs text-muted-foreground">Use TLS</span>
                </label>

                <div className="text-xs font-medium text-muted-foreground uppercase tracking-wide mt-1">
                  SMTP Settings
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <div className="flex flex-col gap-1">
                    <Label htmlFor="add-smtpHost" className="text-xs">Host</Label>
                    <Input
                      id="add-smtpHost"
                      type="text"
                      placeholder="smtp.example.com"
                      disabled={loading}
                      value={serverConfig.smtpHost}
                      onChange={(e) => setServerConfig({ ...serverConfig, smtpHost: e.target.value })}
                      className="h-8 text-sm"
                    />
                  </div>
                  <div className="flex flex-col gap-1">
                    <Label htmlFor="add-smtpPort" className="text-xs">Port</Label>
                    <Input
                      id="add-smtpPort"
                      type="number"
                      placeholder="587"
                      disabled={loading}
                      value={serverConfig.smtpPort}
                      onChange={(e) => setServerConfig({ ...serverConfig, smtpPort: e.target.value })}
                      className="h-8 text-sm"
                    />
                  </div>
                </div>
                <label htmlFor="add-smtpTls" className="flex items-center gap-2 cursor-pointer select-none">
                  <input
                    id="add-smtpTls"
                    type="checkbox"
                    checked={serverConfig.smtpTls}
                    onChange={(e) => setServerConfig({ ...serverConfig, smtpTls: e.target.checked })}
                    disabled={loading}
                    className="size-3 rounded border-muted-foreground/40 accent-primary"
                  />
                  <span className="text-xs text-muted-foreground">Use TLS</span>
                </label>
              </div>
            )}

            <div className="flex justify-end gap-2 mt-2">
              <Button
                type="button"
                variant="outline"
                onClick={onClose}
                disabled={loading}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={loading}
              >
                {loading && <Loader2 className="size-4 animate-spin" />}
                {loading ? "Signing in..." : "Sign in"}
              </Button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
