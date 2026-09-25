// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useCallback, useEffect, useState } from "react";
import {
  Cloud,
  CloudOff,
  Loader2,
  LogIn,
  LogOut,
  Shield,
  UserPlus,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { CloudConfig, CloudSession } from "@/lib/types";
import * as api from "@/lib/tauri";

type AuthMode = "sign-in" | "sign-up";

export function CloudPage() {
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [config, setConfig] = useState<CloudConfig | null>(null);
  const [session, setSession] = useState<CloudSession | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const [authMode, setAuthMode] = useState<AuthMode>("sign-in");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");

  const load = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [cloudConfig, cloudSession] = await Promise.all([
        api.cloudGetConfig(),
        api.cloudGetSession(),
      ]);
      setConfig(cloudConfig);
      setSession(cloudSession);

      // If we have a stale session stub, try to refresh.
      if (cloudSession && !cloudSession.user.id && cloudConfig.configured) {
        try {
          const refreshed = await api.cloudRefreshToken();
          setSession(refreshed);
        } catch {
          setSession(null);
        }
      }
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleAuth() {
    try {
      setBusy(true);
      setError(null);
      setMessage(null);

      const input = { email, password };
      const result =
        authMode === "sign-up"
          ? await api.cloudSignUp(input)
          : await api.cloudSignIn(input);

      setSession(result);
      setEmail("");
      setPassword("");
      setMessage(
        authMode === "sign-up"
          ? "Account created. Check your email if confirmation is required."
          : "Signed in successfully.",
      );
    } catch (authError) {
      setError(String(authError));
    } finally {
      setBusy(false);
    }
  }

  async function handleSignOut() {
    try {
      setBusy(true);
      setError(null);
      setMessage(null);

      await api.cloudSignOut();
      setSession(null);
      setMessage("Signed out.");
    } catch (signOutError) {
      setError(String(signOutError));
    } finally {
      setBusy(false);
    }
  }

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  const isSignedIn = session?.user?.id;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Cloud</h1>
        <p className="text-muted-foreground">
          Backup status, sync, and account management.
        </p>
      </div>

      {error && (
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-sm text-destructive">
          {error}
        </div>
      )}

      {message && (
        <div className="rounded-lg border border-green-500/40 bg-green-500/10 p-4 text-sm text-green-500">
          {message}
        </div>
      )}

      {!config?.configured && (
        <Card className="border border-amber-400/30 bg-amber-500/5">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-amber-200">
              <CloudOff className="h-4 w-4" />
              Cloud Not Configured
            </CardTitle>
            <CardDescription>
              Set the <code className="rounded bg-muted/30 px-1.5 py-0.5 text-xs">VAULTIME_SUPABASE_URL</code> and{" "}
              <code className="rounded bg-muted/30 px-1.5 py-0.5 text-xs">VAULTIME_SUPABASE_ANON_KEY</code> environment
              variables at build time to enable cloud features. Until then, the
              app works fully in local-only mode.
            </CardDescription>
          </CardHeader>
        </Card>
      )}

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
        {!isSignedIn ? (
          <Card className="border border-border/70">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                {authMode === "sign-in" ? (
                  <LogIn className="h-4 w-4 text-primary" />
                ) : (
                  <UserPlus className="h-4 w-4 text-primary" />
                )}
                {authMode === "sign-in" ? "Sign In" : "Create Account"}
              </CardTitle>
              <CardDescription>
                {authMode === "sign-in"
                  ? "Sign in with your Vaultime cloud account to enable backup and sync."
                  : "Create a new account to get started with cloud backup and sync."}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="cloud-email">Email</Label>
                <Input
                  id="cloud-email"
                  type="email"
                  placeholder="you@example.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  disabled={busy || !config?.configured}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="cloud-password">Password</Label>
                <Input
                  id="cloud-password"
                  type="password"
                  placeholder="••••••••"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  disabled={busy || !config?.configured}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && email && password) {
                      void handleAuth();
                    }
                  }}
                />
              </div>
              <div className="flex items-center justify-between gap-4 pt-2">
                <button
                  type="button"
                  className="text-xs text-muted-foreground hover:text-foreground"
                  onClick={() =>
                    setAuthMode(authMode === "sign-in" ? "sign-up" : "sign-in")
                  }
                >
                  {authMode === "sign-in"
                    ? "Need an account? Sign up"
                    : "Already have an account? Sign in"}
                </button>
                <Button
                  onClick={handleAuth}
                  disabled={busy || !email || !password || !config?.configured}
                >
                  {busy ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : authMode === "sign-in" ? (
                    <LogIn className="h-4 w-4" />
                  ) : (
                    <UserPlus className="h-4 w-4" />
                  )}
                  {authMode === "sign-in" ? "Sign In" : "Sign Up"}
                </Button>
              </div>
            </CardContent>
          </Card>
        ) : (
          <Card className="border border-border/70">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Cloud className="h-4 w-4 text-primary" />
                Account
              </CardTitle>
              <CardDescription>
                You are signed in to Vaultime Cloud.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="rounded-2xl border border-border/70 bg-muted/20 px-4 py-3">
                <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                  Email
                </p>
                <p className="mt-1 text-sm font-medium">{session.user.email}</p>
              </div>

              <div className="grid gap-3 sm:grid-cols-2">
                <div className="rounded-2xl border border-border/70 bg-muted/20 px-4 py-3">
                  <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    User ID
                  </p>
                  <p className="mt-1 truncate text-xs text-muted-foreground">
                    {session.user.id}
                  </p>
                </div>
                <div className="rounded-2xl border border-border/70 bg-muted/20 px-4 py-3">
                  <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    Device Registered
                  </p>
                  <Badge
                    variant="outline"
                    className={
                      session.device_registered
                        ? "mt-1 border-green-500/50 text-green-500"
                        : "mt-1 border-yellow-500/50 text-yellow-500"
                    }
                  >
                    {session.device_registered ? "Yes" : "Pending"}
                  </Badge>
                </div>
              </div>

              <div className="flex justify-end pt-2">
                <Button variant="outline" onClick={handleSignOut} disabled={busy}>
                  {busy ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <LogOut className="h-4 w-4" />
                  )}
                  Sign Out
                </Button>
              </div>
            </CardContent>
          </Card>
        )}

        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Shield className="h-4 w-4 text-primary" />
              Cloud Benefits
            </CardTitle>
            <CardDescription>
              What the paid cloud tier adds on top of local-first tracking.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            {[
              {
                title: "Encrypted Cloud Backup",
                description:
                  "Your library, sessions, and artwork are encrypted and stored securely. Restore from any point.",
              },
              {
                title: "Multi-Device Sync",
                description:
                  "Track games across multiple machines and keep a unified session history.",
              },
              {
                title: "Verified Trust Level",
                description:
                  "Server-acknowledged sessions earn a Verified integrity badge instead of Local-only.",
              },
              {
                title: "Backup History",
                description:
                  "Roll back to any previous snapshot. Cloud-stored restore points never expire.",
              },
            ].map((item) => (
              <div
                key={item.title}
                className="rounded-2xl border border-border/70 bg-muted/20 px-4 py-3"
              >
                <p className="text-sm font-medium">{item.title}</p>
                <p className="mt-1 text-xs leading-5 text-muted-foreground">
                  {item.description}
                </p>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
