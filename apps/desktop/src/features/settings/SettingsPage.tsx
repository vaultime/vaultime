// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Loader2, Save, Settings, TimerReset } from "lucide-react";
import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
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
import type { Setting, TrackingDiagnostics } from "@/lib/types";
import * as api from "@/lib/tauri";

function mapSettings(settings: Setting[]): Record<string, string> {
  const values: Record<string, string> = {};
  for (const setting of settings) {
    values[setting.key] = setting.value;
  }
  return values;
}

export function SettingsPage() {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedMessage, setSavedMessage] = useState<string | null>(null);
  const [idleThresholdSeconds, setIdleThresholdSeconds] = useState("300");
  const [treatBackgroundAsActive, setTreatBackgroundAsActive] = useState(false);
  const [diagnostics, setDiagnostics] =
    useState<TrackingDiagnostics | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        setLoading(true);
        setError(null);

        const [settings, trackingDiagnostics] = await Promise.all([
          api.listSettings(),
          api.getTrackingDiagnostics(),
        ]);

        if (cancelled) {
          return;
        }

        const values = mapSettings(settings);
        setIdleThresholdSeconds(values.idle_threshold_seconds ?? "300");
        setTreatBackgroundAsActive(
          values.treat_background_as_active === "true",
        );
        setDiagnostics(trackingDiagnostics);
      } catch (loadError) {
        if (!cancelled) {
          setError(String(loadError));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    load();

    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSave() {
    const parsedThreshold = Number.parseInt(idleThresholdSeconds, 10);
    if (!Number.isFinite(parsedThreshold) || parsedThreshold < 5) {
      setError("Idle threshold must be a whole number of at least 5 seconds.");
      return;
    }

    try {
      setSaving(true);
      setError(null);
      setSavedMessage(null);

      await api.setSetting(
        "idle_threshold_seconds",
        String(parsedThreshold),
      );
      await api.setSetting(
        "treat_background_as_active",
        String(treatBackgroundAsActive),
      );

      setSavedMessage("Tracking rules saved.");
    } catch (saveError) {
      setError(String(saveError));
    } finally {
      setSaving(false);
    }
  }

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Settings</h1>
        <p className="text-muted-foreground">
          Tune active playtime rules and inspect the current tracking signals.
        </p>
      </div>

      {error && (
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-sm text-destructive">
          {error}
        </div>
      )}

      {savedMessage && (
        <div className="rounded-lg border border-green-500/40 bg-green-500/10 p-4 text-sm text-green-500">
          {savedMessage}
        </div>
      )}

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <TimerReset className="h-4 w-4 text-primary" />
              Tracking Rules
            </CardTitle>
            <CardDescription>
              Active time requires a live tracked process and the rules below.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="space-y-2">
              <Label htmlFor="idle-threshold">
                Idle Threshold Seconds
              </Label>
              <Input
                id="idle-threshold"
                type="number"
                min={5}
                step={5}
                value={idleThresholdSeconds}
                onChange={(event) => {
                  setIdleThresholdSeconds(event.target.value);
                  setSavedMessage(null);
                }}
              />
              <p className="text-xs text-muted-foreground">
                When Vaultime sees no convincing activity for this long, it
                shifts the session from active time to idle/background time.
              </p>
            </div>

            <div className="rounded-xl border border-border/70 bg-muted/20 p-4">
              <div className="flex items-start justify-between gap-4">
                <div className="space-y-1">
                  <Label
                    htmlFor="background-active"
                    className="text-sm font-medium"
                  >
                    Count Background Runtime As Active
                  </Label>
                  <p className="text-xs leading-5 text-muted-foreground">
                    Turn this on if you want minimized or unfocused games to
                    keep earning active time while the machine is still in use.
                  </p>
                </div>
                <input
                  id="background-active"
                  type="checkbox"
                  checked={treatBackgroundAsActive}
                  onChange={(event) => {
                    setTreatBackgroundAsActive(event.target.checked);
                    setSavedMessage(null);
                  }}
                  className="mt-0.5 h-4 w-4 rounded border border-input bg-background accent-primary"
                />
              </div>
            </div>

            <div className="flex items-center justify-between gap-4">
              <p className="text-xs text-muted-foreground">
                Poll interval: {diagnostics?.poll_interval_seconds ?? 5}s
              </p>
              <Button onClick={handleSave} disabled={saving}>
                {saving ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Save className="h-4 w-4" />
                )}
                Save Rules
              </Button>
            </div>
          </CardContent>
        </Card>

        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Settings className="h-4 w-4 text-primary" />
              Detection Status
            </CardTitle>
            <CardDescription>
              Backend status and the signal sources currently available to the
              tracker.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between rounded-xl border border-border/70 bg-muted/20 px-4 py-3">
              <span className="text-sm text-muted-foreground">
                Tracking engine
              </span>
              <Badge
                variant="outline"
                className={
                  diagnostics?.running
                    ? "border-green-500/50 text-green-500"
                    : "border-yellow-500/50 text-yellow-500"
                }
              >
                {diagnostics?.running ? "Running" : "Stopped"}
              </Badge>
            </div>

            <div className="rounded-xl border border-border/70 bg-muted/20 p-4">
              <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                Foreground detection
              </p>
              <p className="mt-2 text-lg font-semibold">
                {diagnostics?.foreground_detection === "x11"
                  ? "X11 window PID"
                  : "Process heuristic"}
              </p>
              <p className="mt-1 text-xs leading-5 text-muted-foreground">
                {diagnostics?.foreground_detection === "x11"
                  ? "Vaultime can map the active X11 window back to a tracked game process."
                  : "Vaultime is falling back to recent process activity instead of a direct active-window signal."}
              </p>
            </div>

            <div className="rounded-xl border border-border/70 bg-muted/20 p-4">
              <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                Idle detection
              </p>
              <p className="mt-2 text-lg font-semibold">
                {diagnostics?.idle_detection === "x11"
                  ? "X11 idle timer"
                  : "Process heuristic"}
              </p>
              <p className="mt-1 text-xs leading-5 text-muted-foreground">
                {diagnostics?.idle_detection === "x11"
                  ? "System idle time is available, so background sessions stop counting as active after the configured threshold."
                  : "Vaultime is using process activity gaps as a conservative fallback for idle/background time."}
              </p>
            </div>

            <p className="text-xs leading-5 text-muted-foreground">
              Linux note: `xprop` improves active-window detection on X11 and
              `xprintidle` improves idle detection. Without them, the tracker
              still works but relies more heavily on process heuristics.
            </p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
