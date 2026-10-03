// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { PageRow, PageSection } from "@/components/layout/Page";
import { Input } from "@/components/ui/input";
import { DEVICE_NAME_MAX_CHARS } from "@/lib/constants";
import { ledgerSentence } from "@/lib/sentences";
import * as api from "@/lib/tauri";
import { formatLongDate } from "@/lib/time";
import type { ThisPc } from "@/lib/types";
import { describeError } from "@/lib/utils";

/** The name of this PC and what its ledger vouches for. */
export function ThisPcSection({ onError }: { onError: (message: string | null) => void }) {
  const [pc, setPc] = useState<ThisPc | null>(null);
  const [name, setName] = useState("");

  useEffect(() => {
    let cancelled = false;
    api
      .getThisPc()
      .then((next) => {
        if (cancelled) return;
        setPc(next);
        setName(next.name);
      })
      .catch((error) => onError(describeError(error)));
    return () => {
      cancelled = true;
    };
  }, [onError]);

  async function saveName() {
    if (!pc) return;
    if (name.trim() === "" || name.trim() === pc.name) {
      setName(pc.name);
      return;
    }
    try {
      const next = await api.renameThisPc(name);
      setPc(next);
      setName(next.name);
    } catch (error) {
      onError(describeError(error));
    }
  }

  if (!pc) return null;
  const sound = !pc.ledger.broken && pc.ledger.missing_sessions === 0;
  return (
    <PageSection
      title="This PC"
      description="Vaultime keeps a ledger of the sessions played here, signed with a key that never leaves this PC. It shows when a session is changed or removed outside Vaultime, but it cannot prevent it."
    >
      <PageRow label="Name" htmlFor="pc-name" hint="Tells the sessions played here apart from those of other PCs.">
        <Input
          id="pc-name"
          className="w-56"
          value={name}
          maxLength={DEVICE_NAME_MAX_CHARS}
          onChange={(event) => setName(event.target.value)}
          onBlur={() => void saveName()}
          onKeyDown={(event) => {
            if (event.key === "Enter") event.currentTarget.blur();
          }}
        />
      </PageRow>
      <PageRow
        label="Ledger"
        hint={`${ledgerSentence(pc.ledger)}${pc.ledger.began_at ? ` Kept since ${formatLongDate(pc.ledger.began_at)}.` : ""}`}
      >
        <span className="flex items-center gap-2">
          <span className={sound ? "size-2 rounded-full bg-violet" : "size-2 rounded-full bg-amber"} />
          <span className="text-soft">{pc.ledger.broken ? "Changed" : sound ? "In order" : "Sessions missing"}</span>
        </span>
      </PageRow>
    </PageSection>
  );
}
