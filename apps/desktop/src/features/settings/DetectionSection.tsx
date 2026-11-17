// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { PageRow, PageSection } from "@/components/layout/Page";
import type { TrackingDiagnostics } from "@/lib/types";

const FOREGROUND: Record<string, { title: string; description: string }> = {
  win32_api: { title: "Windows foreground window", description: "Read straight from Windows." },
  x11: { title: "X11 active window", description: "Read straight from X11." },
  macos_system: { title: "macOS frontmost app", description: "Read straight from macOS." },
};

const IDLE: Record<string, { title: string; description: string }> = {
  win32_api: { title: "Windows last input", description: "The time since the last key press or mouse move." },
  x11_input: { title: "X11 input events", description: "The time since the last key press, click or mouse move." },
  x11: { title: "X11 idle timer", description: "The idle time X11 reports." },
  gnome_dbus: { title: "GNOME idle monitor", description: "The time since the last input, from GNOME." },
  macos_ioreg: { title: "macOS input timer", description: "The time since the last input, from macOS." },
};

const CONTROLLERS: Record<string, { title: string; description: string }> = {
  xinput: { title: "XInput", description: "Xbox and other XInput controllers count as input." },
  evdev: { title: "Controller devices", description: "Every connected controller counts as input." },
};

const NO_CONTROLLERS = { title: "Not read", description: "Only the keyboard and mouse count as input here." };

const FALLBACK = {
  title: "Game activity",
  description: "Not available here, so Vaultime goes by what the game does.",
};

/** The signals the tracker reads on this PC. */
export function DetectionSection({
  diagnostics,
  platform,
}: {
  diagnostics: TrackingDiagnostics | null;
  /** The system's name, like "Windows". */
  platform: string;
}) {
  const foreground = FOREGROUND[diagnostics?.foreground_detection ?? ""] ?? FALLBACK;
  const idle = IDLE[diagnostics?.idle_detection ?? ""] ?? FALLBACK;
  const controllers = CONTROLLERS[diagnostics?.controller_detection ?? ""];
  const pollSeconds = diagnostics?.poll_interval_seconds;

  return (
    <PageSection title="Detection" description="How Vaultime tracks play on this PC.">
      <PageRow label="System">{platform}</PageRow>
      <PageRow label="Tracking">
        <span className="flex items-center gap-2">
          <span className={diagnostics?.running ? "size-2 rounded-full bg-violet" : "size-2 rounded-full bg-amber"} />
          {diagnostics?.running ? "Running" : "Stopped"}
        </span>
      </PageRow>
      <PageRow label="Which window is in front" hint={foreground.description}>
        <span className="text-soft">{foreground.title}</span>
      </PageRow>
      <PageRow label="Whether you are there" hint={idle.description}>
        <span className="text-soft">{idle.title}</span>
      </PageRow>
      <PageRow label="Controllers" hint={(controllers ?? NO_CONTROLLERS).description}>
        <span className="text-soft">
          {(controllers ?? NO_CONTROLLERS).title}
          {controllers && diagnostics && (
            <span className="text-faint tabular-nums">, {diagnostics.controllers_connected} connected</span>
          )}
        </span>
      </PageRow>
      {pollSeconds && (
        <PageRow label="Checks every">
          <span className="tabular-nums">{pollSeconds} s</span>
        </PageRow>
      )}
    </PageSection>
  );
}
