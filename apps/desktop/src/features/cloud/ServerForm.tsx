// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useEffectEvent, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, Loader2, Server } from "lucide-react";
import { Notice, PageRow } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useCheckedForm } from "@/features/cloud/checked-form";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { useSettings } from "@/features/settings/settings-context";
import { SETTING_KEYS, VAULTIME_URL } from "@/lib/constants";
import { formatServerAddress } from "./backups";

/** The page of the documentation on setting up a server. */
const OWN_SERVER_DOCS = `${VAULTIME_URL}/docs/own-server/`;

/** Picks the server to sign in to, the one of the last sign-in to begin with. */
export function ServerForm() {
  const { server, forgetServer } = useCloudSession();
  const saved = useSettings().values[SETTING_KEYS.cloudServer] ?? "";

  if (server) {
    return (
      <PageRow
        label="Server"
        hint={server.signups_open ? "It takes new accounts." : "It takes no new accounts. Sign in with one you have there."}
      >
        <div className="flex items-center gap-3">
          <span className="font-mono text-sm">{formatServerAddress(server.address)}</span>
          <Button variant="ghost" size="sm" onClick={forgetServer}>
            Change
          </Button>
        </div>
      </PageRow>
    );
  }
  // Starts again once the settings are read, with the saved address.
  return <AddressForm key={saved} saved={saved} />;
}

function AddressForm({ saved }: { saved: string }) {
  const { connectServer, reconnectServer } = useCloudSession();
  const [address, setAddress] = useState(saved);
  const form = useCheckedForm({ "cloud-server": address.trim() ? null : "Enter the address of the server." });

  // The server of the last sign-in comes back by itself. When it does not
  // answer, Connect says why.
  const reconnect = useEffectEvent(reconnectServer);
  useEffect(() => {
    reconnect();
  }, []);

  return (
    <form
      className="grid max-w-[520px] gap-4"
      onSubmit={(event) =>
        form.submit(event, async () => {
          await connectServer(address);
        })
      }
    >
      <Field
        id="cloud-server"
        label="Server address"
        hint="Such as vaultime.example.com. Vaultime reaches it over https."
        problem={form.problem("cloud-server")}
      >
        <Input
          id="cloud-server"
          className="font-mono"
          autoComplete="url"
          spellCheck={false}
          placeholder="vaultime.example.com"
          value={address}
          onChange={(event) => setAddress(event.target.value)}
        />
      </Field>
      {form.error && <Notice tone="warning">{form.error}</Notice>}
      <div className="flex flex-wrap gap-2">
        <Button type="submit" disabled={form.busy}>
          {form.busy ? <Loader2 className="size-4 animate-spin" /> : <Server className="size-4" />}
          {form.busy ? "Connecting" : "Connect"}
        </Button>
        <Button type="button" variant="ghost" onClick={() => void openUrl(OWN_SERVER_DOCS)}>
          <ExternalLink className="size-3.5" />
          Set up a server
        </Button>
      </div>
    </form>
  );
}
