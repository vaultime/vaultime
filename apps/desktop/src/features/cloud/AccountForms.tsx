// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState } from "react";
import { Loader2 } from "lucide-react";
import { Notice } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { PasswordInput } from "@/components/ui/password-input";
import { useCheckedForm } from "@/features/cloud/checked-form";
import { useCloudSession } from "@/features/cloud/cloud-context";
import {
  BACKUP_PASSPHRASE_HINT,
  PASSWORD_HINT,
  backupPassphraseProblem,
  emailProblem,
  enteredPasswordProblem,
  inviteCodeProblem,
  newPasswordProblem,
  repeatProblem,
} from "@/features/cloud/credentials";
import { INVITE_CODE_PREFIX } from "@/lib/constants";

const FORM_CLASS = "grid max-w-[520px] gap-4";

export function SignInForm({ onDone }: { onDone: (message: string) => void }) {
  const { login } = useCloudSession();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const form = useCheckedForm({
    "cloud-login-email": emailProblem(email),
    "cloud-login-password": enteredPasswordProblem(password),
  });

  return (
    <form
      className={FORM_CLASS}
      onSubmit={(event) =>
        form.submit(event, async () => {
          const session = await login(email, password);
          onDone(`Signed in as ${session.user.email}.`);
        })
      }
    >
      <Field id="cloud-login-email" label="Email" problem={form.problem("cloud-login-email")}>
        <Input id="cloud-login-email" autoComplete="email" value={email} onChange={(event) => setEmail(event.target.value)} />
      </Field>
      <Field id="cloud-login-password" label="Password" problem={form.problem("cloud-login-password")}>
        <PasswordInput
          id="cloud-login-password"
          autoComplete="current-password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
      </Field>
      {form.error && <Notice tone="warning">{form.error}</Notice>}
      <div>
        <Button type="submit" disabled={form.busy}>
          {form.busy && <Loader2 className="size-4 animate-spin" />}
          {form.busy ? "Signing in" : "Sign in"}
        </Button>
      </div>
    </form>
  );
}

export function SignUpForm({ onDone }: { onDone: (message: string) => void }) {
  const { signUp } = useCloudSession();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [inviteCode, setInviteCode] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [passphraseAgain, setPassphraseAgain] = useState("");
  const form = useCheckedForm({
    "cloud-signup-email": emailProblem(email),
    "cloud-signup-password": newPasswordProblem(password),
    "cloud-signup-invite": inviteCodeProblem(inviteCode),
    "cloud-signup-backup-passphrase": backupPassphraseProblem(passphrase),
    "cloud-signup-backup-passphrase-confirm": repeatProblem(passphrase, passphraseAgain, "backup passphrase"),
  });

  return (
    <form
      className={FORM_CLASS}
      onSubmit={(event) =>
        form.submit(event, async () => {
          const session = await signUp(email, password, inviteCode, passphrase);
          onDone(`Cloud account created for ${session.user.email}.`);
        })
      }
    >
      <Field id="cloud-signup-email" label="Email" problem={form.problem("cloud-signup-email")}>
        <Input id="cloud-signup-email" autoComplete="email" value={email} onChange={(event) => setEmail(event.target.value)} />
      </Field>
      <Field id="cloud-signup-password" label="Password" hint={PASSWORD_HINT} problem={form.problem("cloud-signup-password")}>
        <PasswordInput
          id="cloud-signup-password"
          autoComplete="new-password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
      </Field>
      <Field id="cloud-signup-invite" label="Invite code" problem={form.problem("cloud-signup-invite")}>
        <Input
          id="cloud-signup-invite"
          // Six groups of four, INVITE_BODY_CHARS in the API's constants.rs.
          placeholder={`${INVITE_CODE_PREFIX}-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`}
          value={inviteCode}
          onChange={(event) => setInviteCode(event.target.value)}
          className="font-mono"
        />
      </Field>
      <Field
        id="cloud-signup-backup-passphrase"
        label="Backup passphrase"
        hint={BACKUP_PASSPHRASE_HINT}
        problem={form.problem("cloud-signup-backup-passphrase")}
      >
        <PasswordInput
          id="cloud-signup-backup-passphrase"
          autoComplete="new-password"
          value={passphrase}
          onChange={(event) => setPassphrase(event.target.value)}
        />
      </Field>
      <Field
        id="cloud-signup-backup-passphrase-confirm"
        label="Backup passphrase again"
        problem={form.problem("cloud-signup-backup-passphrase-confirm")}
      >
        <PasswordInput
          id="cloud-signup-backup-passphrase-confirm"
          autoComplete="new-password"
          value={passphraseAgain}
          onChange={(event) => setPassphraseAgain(event.target.value)}
        />
      </Field>
      {form.error && <Notice tone="warning">{form.error}</Notice>}
      <div>
        <Button type="submit" disabled={form.busy}>
          {form.busy && <Loader2 className="size-4 animate-spin" />}
          {form.busy ? "Creating the account" : "Create account"}
        </Button>
      </div>
    </form>
  );
}

export function UnlockBackupsForm({ onDone }: { onDone: (message: string) => void }) {
  const { setBackupPassphrase } = useCloudSession();
  const [passphrase, setPassphrase] = useState("");
  const [passphraseAgain, setPassphraseAgain] = useState("");
  const form = useCheckedForm({
    "device-backup-passphrase": backupPassphraseProblem(passphrase),
    "device-backup-passphrase-confirm": repeatProblem(passphrase, passphraseAgain, "backup passphrase"),
  });

  return (
    <form
      className={`mb-8 ${FORM_CLASS}`}
      onSubmit={(event) =>
        form.submit(event, async () => {
          await setBackupPassphrase(passphrase);
          onDone("Backup passphrase unlocked for this PC.");
        })
      }
    >
      <p className="text-sm leading-relaxed text-soft">
        New account: choose a backup passphrase now. Existing backups: enter the passphrase you used for them.
      </p>
      <Field
        id="device-backup-passphrase"
        label="Backup passphrase"
        hint={BACKUP_PASSPHRASE_HINT}
        problem={form.problem("device-backup-passphrase")}
      >
        <PasswordInput
          id="device-backup-passphrase"
          autoComplete="new-password"
          value={passphrase}
          onChange={(event) => setPassphrase(event.target.value)}
        />
      </Field>
      <Field
        id="device-backup-passphrase-confirm"
        label="Backup passphrase again"
        problem={form.problem("device-backup-passphrase-confirm")}
      >
        <PasswordInput
          id="device-backup-passphrase-confirm"
          autoComplete="new-password"
          value={passphraseAgain}
          onChange={(event) => setPassphraseAgain(event.target.value)}
        />
      </Field>
      {form.error && <Notice tone="warning">{form.error}</Notice>}
      <div>
        <Button type="submit" disabled={form.busy}>
          {form.busy && <Loader2 className="size-4 animate-spin" />}
          {form.busy ? "Unlocking" : "Unlock backups on this PC"}
        </Button>
      </div>
    </form>
  );
}
