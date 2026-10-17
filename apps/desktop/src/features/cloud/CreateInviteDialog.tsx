// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState, type FormEvent } from "react";
import { Copy, KeyRound, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { INVITE_CODE_PREFIX } from "@/lib/constants";
import type { CloudAdminInvite } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { plural } from "@/lib/words";
import { formatTimestamp, toIsoTimestamp } from "./backups";

/** Makes an invite code for a new cloud account. Only admins get here. */
export function CreateInviteDialog({
  open,
  onOpenChange,
  onStatus,
  onError,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** What the page says above, a success or null to clear it. */
  onStatus: (message: string | null) => void;
  onError: (message: string | null) => void;
}) {
  const { createAdminInvite } = useCloudSession();
  const [prefix, setPrefix] = useState(INVITE_CODE_PREFIX);
  const [maxRedemptions, setMaxRedemptions] = useState("1");
  const [expiry, setExpiry] = useState("");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [lastInvite, setLastInvite] = useState<CloudAdminInvite | null>(null);
  const [copied, setCopied] = useState(false);

  async function generate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setCopied(false);
    onError(null);
    onStatus(null);

    try {
      const uses = Number.parseInt(maxRedemptions, 10);
      if (!Number.isFinite(uses) || uses < 1) {
        throw new Error("Max redemptions must be at least 1.");
      }

      const generated = await createAdminInvite({
        prefix: prefix.trim() || null,
        max_redemptions: uses,
        expires_at: toIsoTimestamp(expiry),
        note: note.trim() || null,
      });

      setLastInvite(generated);
      onStatus("New invite key generated.");
    } catch (error) {
      onError(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function copy() {
    if (!lastInvite?.code) {
      return;
    }

    try {
      await navigator.clipboard.writeText(lastInvite.code);
      setCopied(true);
      onStatus("Invite code copied.");
    } catch (error) {
      onError(describeError(error));
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Create an invite</DialogTitle>
          <DialogDescription>A code someone can use to create a cloud account.</DialogDescription>
        </DialogHeader>

        <form className="grid gap-4" onSubmit={generate}>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field id="invite-prefix" label="Prefix">
              <Input
                id="invite-prefix"
                value={prefix}
                onChange={(event) => setPrefix(event.target.value)}
                className="font-mono"
              />
            </Field>
            <Field id="invite-max-redemptions" label="Uses">
              <Input
                id="invite-max-redemptions"
                inputMode="numeric"
                value={maxRedemptions}
                onChange={(event) => setMaxRedemptions(event.target.value)}
              />
            </Field>
            <Field id="invite-expiry" label="Expires">
              <Input
                id="invite-expiry"
                type="datetime-local"
                value={expiry}
                onChange={(event) => setExpiry(event.target.value)}
              />
            </Field>
            <Field id="invite-note" label="Note">
              <Input
                id="invite-note"
                placeholder="beta tester"
                value={note}
                onChange={(event) => setNote(event.target.value)}
              />
            </Field>
          </div>

          {lastInvite && (
            <div className="border-l-2 border-violet py-1 pl-4">
              <div className="label-caps">Share this code</div>
              <div className="mt-2 flex flex-wrap items-center justify-between gap-3">
                <span className="font-mono text-base break-all">{lastInvite.code}</span>
                <Button type="button" variant="outline" size="sm" onClick={copy}>
                  <Copy className="size-3.5" />
                  {copied ? "Copied" : "Copy"}
                </Button>
              </div>
              <p className="mt-2 text-[13px] text-faint">
                {plural(lastInvite.max_redemptions, "use")},{" "}
                {lastInvite.expires_at ? `expires ${formatTimestamp(lastInvite.expires_at)}` : "never expires"}
              </p>
            </div>
          )}

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              Close
            </Button>
            <Button type="submit" disabled={busy}>
              {busy ? <Loader2 className="size-4 animate-spin" /> : <KeyRound className="size-4" />}
              {busy ? "Creating" : "Create invite"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
