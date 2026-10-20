// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { PageRow, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import type { CloudDevice } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { capitalize } from "@/lib/words";
import { formatTimestamp } from "./backups";
import { useCloudSession } from "./cloud-context";

/** Every PC that signed in with the account, and removing the ones gone. */
export function RegisteredPcsSection({ onError }: { onError: (message: string | null) => void }) {
  const { device, listDevices, removeDevice } = useCloudSession();
  const [devices, setDevices] = useState<CloudDevice[] | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);

  // Read again when this PC registers, which changes the list.
  const thisPc = device?.id ?? null;
  useEffect(() => {
    let cancelled = false;
    listDevices()
      .then((listed) => {
        if (!cancelled) setDevices(listed);
      })
      .catch((error) => {
        if (!cancelled) onError(describeError(error));
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the provider makes its functions anew on every render
  }, [thisPc]);

  async function remove(id: string) {
    onError(null);
    setRemoving(id);
    try {
      await removeDevice(id);
      setDevices((listed) => listed?.filter((each) => each.id !== id) ?? null);
    } catch (error) {
      onError(describeError(error));
    } finally {
      setRemoving(null);
    }
  }

  if (!devices || devices.length === 0) return null;
  return (
    <PageSection
      title="Registered PCs"
      description="Every PC that signed in with this account. Removing one frees its place, and its backups stay. A PC still in use registers again when it next signs in."
    >
      {devices.map((each) => (
        <PageRow
          key={each.id}
          label={each.id === thisPc ? `${each.device_name}, this PC` : each.device_name}
          hint={`${capitalize(each.platform)}, Vaultime ${each.app_version}, last seen ${formatTimestamp(each.last_seen_at)}.`}
        >
          {each.id !== thisPc && (
            <Button variant="outline" size="sm" disabled={removing !== null} onClick={() => void remove(each.id)}>
              {removing === each.id ? "Removing…" : "Remove"}
            </Button>
          )}
        </PageRow>
      ))}
    </PageSection>
  );
}
