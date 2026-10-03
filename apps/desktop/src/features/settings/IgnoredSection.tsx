// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { Undo2 } from "lucide-react";
import { PageRow, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import * as api from "@/lib/tauri";
import type { IgnoredProgram } from "@/lib/types";
import { describeError } from "@/lib/utils";

/** Programs the player said are no game, each with a way back. Hidden while there are none. */
export function IgnoredSection({ onError }: { onError: (message: string | null) => void }) {
  const [programs, setPrograms] = useState<IgnoredProgram[]>([]);

  useEffect(() => {
    let cancelled = false;
    api
      .listIgnoredPrograms()
      .then((next) => {
        if (!cancelled) setPrograms(next);
      })
      .catch((error) => onError(describeError(error)));
    return () => {
      cancelled = true;
    };
  }, [onError]);

  async function allow(program: IgnoredProgram) {
    try {
      await api.allowProgram(program.path_key);
      setPrograms((current) => current.filter((entry) => entry.path_key !== program.path_key));
    } catch (error) {
      onError(describeError(error));
    }
  }

  if (programs.length === 0) return null;
  return (
    <PageSection
      title="Ignored programs"
      description="Not games. Discover never offers them, and Vaultime never tracks them, also inside a game's folder."
    >
      {programs.map((program) => (
        <PageRow
          key={program.path_key}
          label={program.title}
          hint={<span className="font-mono text-[11px] break-all">{program.path}</span>}
        >
          <Button variant="outline" size="sm" onClick={() => void allow(program)}>
            <Undo2 className="size-3.5" />
            Allow again
          </Button>
        </PageRow>
      ))}
    </PageSection>
  );
}
