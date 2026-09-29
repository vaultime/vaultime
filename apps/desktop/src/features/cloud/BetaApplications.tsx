// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useEffectEvent, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Copy, Loader2, Mail, Trash2 } from "lucide-react";
import { PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { formatSessionStart } from "@/lib/time";
import type { BetaApplication } from "@/lib/types";
import { describeError } from "@/lib/utils";

const PLATFORM_NAMES: Record<string, string> = { windows: "Windows", linux: "Linux", both: "Windows and Linux" };

/** An email to the applicant with the invite code, for the default mail program. */
function inviteMail(email: string, code: string): string {
  const subject = "Your invite to the Vaultime cloud beta";
  const body = [
    "Hi,",
    "",
    "thanks for applying for the Vaultime cloud beta. Here is your invite code, it works once:",
    "",
    code,
    "",
    "In Vaultime, open Cloud, choose Create account and enter the code with your email address and a password.",
    "",
    "Vaultime",
  ].join("\n");
  const address = encodeURIComponent(email).replace("%40", "@");
  return `mailto:${address}?subject=${encodeURIComponent(subject)}&body=${encodeURIComponent(body)}`;
}

/**
 * Applications from the website, for admins. Inviting creates a code for one
 * account, opens an email to the applicant and removes the application.
 */
export function BetaApplications({ onError }: { onError: (message: string) => void }) {
  const { listBetaApplications, deleteBetaApplication, createAdminInvite } = useCloudSession();
  const [applications, setApplications] = useState<BetaApplication[] | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [invited, setInvited] = useState<{ email: string; code: string } | null>(null);
  const [copied, setCopied] = useState(false);

  const load = useEffectEvent(() => {
    listBetaApplications()
      .then(setApplications)
      .catch((error: unknown) => onError(describeError(error)));
  });
  useEffect(() => load(), []);

  function remove(id: string) {
    setApplications((list) => list?.filter((application) => application.id !== id) ?? null);
  }

  async function invite(application: BetaApplication) {
    setBusyId(application.id);
    setCopied(false);
    try {
      const created = await createAdminInvite({ max_redemptions: 1, note: `Beta: ${application.email}` });
      await deleteBetaApplication(application.id);
      remove(application.id);
      setInvited({ email: application.email, code: created.code });
      await openUrl(inviteMail(application.email, created.code));
    } catch (error) {
      onError(describeError(error));
    } finally {
      setBusyId(null);
    }
  }

  async function decline(application: BetaApplication) {
    setBusyId(application.id);
    try {
      await deleteBetaApplication(application.id);
      remove(application.id);
    } catch (error) {
      onError(describeError(error));
    } finally {
      setBusyId(null);
    }
  }

  async function copyCode() {
    if (!invited) return;
    try {
      await navigator.clipboard.writeText(invited.code);
      setCopied(true);
    } catch (error) {
      onError(describeError(error));
    }
  }

  return (
    <PageSection
      title="Beta applications"
      description="From the form on the website. Invite creates a code for one account, opens an email to the applicant and removes the application."
    >
      {invited && (
        <div className="mb-4 border-l-2 border-violet py-1 pl-4">
          <div className="label-caps">Invite for {invited.email}</div>
          <div className="mt-2 flex flex-wrap items-center justify-between gap-3">
            <span className="font-mono text-base break-all">{invited.code}</span>
            <Button variant="outline" size="sm" onClick={() => void copyCode()}>
              <Copy className="size-3.5" />
              {copied ? "Copied" : "Copy"}
            </Button>
          </div>
          <p className="mt-2 text-[13px] text-faint">Shown once. If no email opened, send the code yourself.</p>
        </div>
      )}

      {applications === null ? (
        <p className="flex items-center gap-2 py-3 text-sm text-faint">
          <Loader2 className="size-4 animate-spin" />
          Loading applications
        </p>
      ) : applications.length === 0 ? (
        <p className="py-3 text-sm text-faint">No applications right now.</p>
      ) : (
        applications.map((application) => (
          <div
            key={application.id}
            className="flex flex-wrap items-baseline gap-x-5 gap-y-2 border-b border-rule py-3.5 last:border-b-0"
          >
            <span className="w-[20ch] shrink-0 font-mono text-[13px] text-faint">
              {formatSessionStart(application.created_at)}
            </span>
            <div className="min-w-0 flex-1">
              <div className="text-[15px] break-all">{application.email}</div>
              <div className="mt-0.5 text-[13px] text-faint">
                Plays on {PLATFORM_NAMES[application.platform] ?? application.platform}
              </div>
              {application.note && <p className="mt-1.5 text-[13px] whitespace-pre-line text-soft">{application.note}</p>}
            </div>
            <div className="flex gap-1">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void invite(application)}
                disabled={busyId !== null}
              >
                {busyId === application.id ? <Loader2 className="size-3.5 animate-spin" /> : <Mail className="size-3.5" />}
                Invite
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={`Delete the application of ${application.email}`}
                onClick={() => void decline(application)}
                disabled={busyId !== null}
              >
                <Trash2 className="size-3.5" />
              </Button>
            </div>
          </div>
        ))
      )}
    </PageSection>
  );
}
