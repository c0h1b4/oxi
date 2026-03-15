"use client";

import { useEffect, useState, type FormEvent } from "react";
import { useRouter } from "next/navigation";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { apiPost, fetchAccounts } from "@/lib/api";
import { useAuthStore } from "@/stores/useAuthStore";

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

export default function Home() {
  const router = useRouter();
  const setAccounts = useAuthStore((s) => s.setAccounts);
  const setActiveAccount = useAuthStore((s) => s.setActiveAccount);
  const [checking, setChecking] = useState(true);
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

  useEffect(() => {
    let cancelled = false;
    fetchAccounts()
      .then((data) => {
        if (!cancelled) {
          if (data.accounts.length > 0) {
            router.replace("/mail");
          } else {
            setChecking(false);
          }
        }
      })
      .catch(() => {
        if (!cancelled) setChecking(false);
      });
    return () => {
      cancelled = true;
    };
  }, [router]);

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
      router.push("/mail");
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "An unexpected error occurred",
      );
    } finally {
      setLoading(false);
    }
  }

  if (checking) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background">
        <div className="size-8 animate-spin rounded-full border-4 border-muted border-t-primary" />
      </div>
    );
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-background px-4">
      <div className="w-full max-w-sm">
        {/* Branding */}
        <div className="mb-8 text-center">
          <h1 className="text-3xl font-bold tracking-tight text-foreground">
            oxi<span className="text-primary">.email</span>
          </h1>
          <p className="mt-2 text-sm text-muted-foreground">
            Sign in to your account
          </p>
        </div>

        {/* Login Card */}
        <Card className="rounded-lg">
          <CardHeader>
            <CardTitle className="text-lg">Welcome back</CardTitle>
            <CardDescription>
              Enter your credentials to continue
            </CardDescription>
          </CardHeader>
          <CardContent>
            {error && (
              <div className="mb-4 rounded-md bg-destructive/10 px-4 py-3 text-sm text-destructive">
                {error}
              </div>
            )}
            <form onSubmit={handleSubmit} className="flex flex-col gap-4">
              <div className="flex flex-col gap-2">
                <Label htmlFor="email">Email</Label>
                <Input
                  id="email"
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
                <Label htmlFor="password">Password</Label>
                <Input
                  id="password"
                  type="password"
                  placeholder="Enter your password"
                  autoComplete="current-password"
                  required
                  disabled={loading}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
              </div>
              <label htmlFor="remember" className="flex items-center gap-2 cursor-pointer select-none">
                <input
                  id="remember"
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
                      <Label htmlFor="imapHost" className="text-xs">Host</Label>
                      <Input
                        id="imapHost"
                        type="text"
                        placeholder="imap.example.com"
                        disabled={loading}
                        value={serverConfig.imapHost}
                        onChange={(e) => setServerConfig({ ...serverConfig, imapHost: e.target.value })}
                        className="h-8 text-sm"
                      />
                    </div>
                    <div className="flex flex-col gap-1">
                      <Label htmlFor="imapPort" className="text-xs">Port</Label>
                      <Input
                        id="imapPort"
                        type="number"
                        placeholder="993"
                        disabled={loading}
                        value={serverConfig.imapPort}
                        onChange={(e) => setServerConfig({ ...serverConfig, imapPort: e.target.value })}
                        className="h-8 text-sm"
                      />
                    </div>
                  </div>
                  <label htmlFor="imapTls" className="flex items-center gap-2 cursor-pointer select-none">
                    <input
                      id="imapTls"
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
                      <Label htmlFor="smtpHost" className="text-xs">Host</Label>
                      <Input
                        id="smtpHost"
                        type="text"
                        placeholder="smtp.example.com"
                        disabled={loading}
                        value={serverConfig.smtpHost}
                        onChange={(e) => setServerConfig({ ...serverConfig, smtpHost: e.target.value })}
                        className="h-8 text-sm"
                      />
                    </div>
                    <div className="flex flex-col gap-1">
                      <Label htmlFor="smtpPort" className="text-xs">Port</Label>
                      <Input
                        id="smtpPort"
                        type="number"
                        placeholder="587"
                        disabled={loading}
                        value={serverConfig.smtpPort}
                        onChange={(e) => setServerConfig({ ...serverConfig, smtpPort: e.target.value })}
                        className="h-8 text-sm"
                      />
                    </div>
                  </div>
                  <label htmlFor="smtpTls" className="flex items-center gap-2 cursor-pointer select-none">
                    <input
                      id="smtpTls"
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

              <Button
                type="submit"
                className="mt-2 w-full"
                size="lg"
                disabled={loading}
              >
                {loading ? "Signing in..." : "Sign in"}
              </Button>
            </form>
          </CardContent>
        </Card>

        {/* Footer */}
        <p className="mt-6 text-center text-xs text-muted-foreground">
          Secure, private, and fast email.
        </p>
      </div>
    </div>
  );
}
