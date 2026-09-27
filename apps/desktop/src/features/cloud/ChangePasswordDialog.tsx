// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useState, type FormEvent } from "react";
import { Loader2 } from "lucide-react";
import { Notice } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { Field } from "@/features/cloud/Field";
import { describeError } from "@/lib/utils";

export function ChangePasswordDialog({
  open,
  onOpenChange,
  onChanged,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onChanged: () => void;
}) {
  const { changePassword } = useCloudSession();
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [newPasswordConfirm, setNewPasswordConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Start over every time the dialog opens.
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) {
      setCurrentPassword("");
      setNewPassword("");
      setNewPasswordConfirm("");
      setBusy(false);
      setError(null);
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (newPassword !== newPasswordConfirm) {
      setError("The new passwords do not match.");
      return;
    }
    try {
      setBusy(true);
      setError(null);
      await changePassword(currentPassword, newPassword);
      onOpenChange(false);
      onChanged();
    } catch (changeError) {
      setError(describeError(changeError));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Change password</DialogTitle>
          <DialogDescription>
            Your other PCs are signed out and need the new password. The backup passphrase stays the same.
          </DialogDescription>
        </DialogHeader>

        {error && <Notice tone="warning">{error}</Notice>}

        <form className="grid gap-4" onSubmit={handleSubmit}>
          <Field id="cloud-current-password" label="Current password">
            <Input
              id="cloud-current-password"
              type="password"
              autoComplete="current-password"
              value={currentPassword}
              onChange={(event) => setCurrentPassword(event.target.value)}
            />
          </Field>
          <Field id="cloud-new-password" label="New password">
            <Input
              id="cloud-new-password"
              type="password"
              autoComplete="new-password"
              value={newPassword}
              onChange={(event) => setNewPassword(event.target.value)}
            />
          </Field>
          <Field id="cloud-new-password-confirm" label="New password again">
            <Input
              id="cloud-new-password-confirm"
              type="password"
              autoComplete="new-password"
              value={newPasswordConfirm}
              onChange={(event) => setNewPasswordConfirm(event.target.value)}
            />
          </Field>

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy || !currentPassword || !newPassword || !newPasswordConfirm}>
              {busy && <Loader2 className="size-4 animate-spin" />}
              {busy ? "Changing" : "Change password"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
