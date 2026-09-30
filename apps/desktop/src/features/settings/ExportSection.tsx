// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState } from "react";
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import { Braces, FileSpreadsheet, Loader2 } from "lucide-react";
import { Notice, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { toDayKey } from "@/lib/session-stats";
import * as api from "@/lib/tauri";
import type { ExportFormat } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { plural } from "@/lib/words";

const FORMATS: { format: ExportFormat; label: string; filter: string; icon: typeof Braces }[] = [
  { format: "csv", label: "Export as CSV", filter: "CSV", icon: FileSpreadsheet },
  { format: "json", label: "Export as JSON", filter: "JSON", icon: Braces },
];

/** Every finished session as a file for a spreadsheet or other tools. */
export function ExportSection() {
  const [busy, setBusy] = useState<ExportFormat | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function run(format: ExportFormat, filter: string) {
    setMessage(null);
    setError(null);
    try {
      const path = await saveFileDialog({
        defaultPath: `vaultime-sessions-${toDayKey(new Date())}.${format}`,
        filters: [{ name: filter, extensions: [format] }],
      });
      if (!path) return;
      setBusy(format);
      const count = await api.exportSessions(path, format);
      setMessage(`Saved ${plural(count, "session")} to ${path}.`);
    } catch (exportError) {
      setError(describeError(exportError));
    } finally {
      setBusy(null);
    }
  }

  return (
    <PageSection
      title="Export"
      description="Your sessions as a file of your own. CSV opens in any spreadsheet, JSON adds your games with their status and earlier playtime."
    >
      <div className="flex flex-wrap gap-3">
        {FORMATS.map(({ format, label, filter, icon: Icon }) => (
          <Button key={format} variant="outline" onClick={() => void run(format, filter)} disabled={busy !== null}>
            {busy === format ? <Loader2 className="size-4 animate-spin" /> : <Icon className="size-4" />}
            {label}
          </Button>
        ))}
      </div>
      {message && <Notice className="mt-5">{message}</Notice>}
      {error && (
        <Notice tone="warning" className="mt-5">
          {error}
        </Notice>
      )}
    </PageSection>
  );
}
