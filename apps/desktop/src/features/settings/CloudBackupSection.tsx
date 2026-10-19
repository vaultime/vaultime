// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Link } from "react-router";
import { PageRow, PageSection } from "@/components/layout/Page";
import { Switch } from "@/components/ui/switch";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { useSettings } from "@/features/settings/settings-context";
import { SETTING_KEYS } from "@/lib/constants";
import { describeError } from "@/lib/utils";

/** Whether this PC uses cloud backup, which brings the Cloud page into the menu. */
export function CloudBackupSection({ onError }: { onError: (message: string | null) => void }) {
  const { enabled, session } = useCloudSession();
  const { save } = useSettings();
  const signedIn = session !== null;

  async function change(next: boolean) {
    try {
      onError(null);
      await save(SETTING_KEYS.cloudBackup, String(next));
    } catch (saveError) {
      onError(describeError(saveError));
    }
  }

  return (
    <PageSection
      title="Cloud backup"
      description="Encrypted backups on our own server, for invited accounts. Vaultime works fully without it."
    >
      <PageRow
        label="Use cloud backup"
        htmlFor="use-cloud-backup"
        hint={
          signedIn ? (
            <>
              This PC is signed in. Sign out on the{" "}
              <Link to="/cloud" className="underline underline-offset-2 hover:text-soft">
                Cloud page
              </Link>{" "}
              to turn it off.
            </>
          ) : enabled ? (
            "The Cloud page in the menu signs this PC in, or signs up with an invite."
          ) : (
            "Off, the Cloud page stays out of the menu and this PC neither signs in nor uploads backups."
          )
        }
      >
        <Switch
          id="use-cloud-backup"
          checked={enabled}
          disabled={signedIn}
          onCheckedChange={(checked) => void change(checked)}
        />
      </PageRow>
    </PageSection>
  );
}
