// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Download, Loader2, RefreshCw } from "lucide-react";
import { PageRow } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { useUpdates } from "@/features/updates/update-context";
import { formatClockTime } from "@/lib/time";

/** Where the update check stands, with a button to look now or to install. */
export function UpdatesRow() {
  const { status, version, error, checkedAt, check, install } = useUpdates();

  const hint =
    status === "checking"
      ? "Looking for a new version."
      : status === "installing"
        ? `Installing version ${version}. Vaultime restarts when it is done.`
        : status === "failed"
          ? `That did not work: ${error ?? "no answer from the update server"}.`
          : status === "available"
            ? `Version ${version} is ready to install.`
            : status === "current" && checkedAt
              ? `Vaultime is up to date. Checked at ${formatClockTime(checkedAt)}.`
              : "Vaultime looks for updates when it starts and every few hours.";

  const busy = status === "checking" || status === "installing";
  return (
    <PageRow label="Updates" hint={<span className={status === "failed" ? "text-amber" : undefined}>{hint}</span>}>
      {version && status !== "checking" ? (
        <Button size="sm" onClick={() => void install()} disabled={busy}>
          {status === "installing" ? <Loader2 className="size-3.5 animate-spin" /> : <Download className="size-3.5" />}
          Update and restart
        </Button>
      ) : (
        <Button variant="outline" size="sm" onClick={() => void check()} disabled={busy}>
          {busy ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
          Check now
        </Button>
      )}
    </PageRow>
  );
}
