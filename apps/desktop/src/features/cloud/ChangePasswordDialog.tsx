// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState } from "react";
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
import { Field } from "@/components/ui/field";
import { PasswordInput } from "@/components/ui/password-input";
import { useCheckedForm } from "@/features/cloud/checked-form";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { PASSWORD_HINT, enteredPasswordProblem, newPasswordProblem, repeatProblem } from "@/features/cloud/credentials";

export function ChangePasswordDialog({
  open,
  onOpenChange,
  onChanged,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onChanged: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Change password</DialogTitle>
          <DialogDescription>
            Your other PCs are signed out and need the new password. The backup passphrase stays the same.
          </DialogDescription>
        </DialogHeader>
        {/* The content unmounts when the dialog closes, so every opening starts empty. */}
        <ChangePasswordForm
          onCancel={() => onOpenChange(false)}
          onChanged={() => {
            onOpenChange(false);
            onChanged();
          }}
        />
      </DialogContent>
    </Dialog>
  );
}

function ChangePasswordForm({ onCancel, onChanged }: { onCancel: () => void; onChanged: () => void }) {
  const { changePassword } = useCloudSession();
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [newPasswordAgain, setNewPasswordAgain] = useState("");
  const form = useCheckedForm({
    "cloud-current-password": enteredPasswordProblem(currentPassword, "current password"),
    "cloud-new-password": newPasswordProblem(newPassword),
    "cloud-new-password-confirm": repeatProblem(newPassword, newPasswordAgain, "new password"),
  });

  return (
    <form
      className="grid gap-4"
      onSubmit={(event) =>
        form.submit(event, async () => {
          await changePassword(currentPassword, newPassword);
          onChanged();
        })
      }
    >
      <Field id="cloud-current-password" label="Current password" problem={form.problem("cloud-current-password")}>
        <PasswordInput
          id="cloud-current-password"
          autoComplete="current-password"
          value={currentPassword}
          onChange={(event) => setCurrentPassword(event.target.value)}
        />
      </Field>
      <Field
        id="cloud-new-password"
        label="New password"
        hint={PASSWORD_HINT}
        problem={form.problem("cloud-new-password")}
      >
        <PasswordInput
          id="cloud-new-password"
          autoComplete="new-password"
          value={newPassword}
          onChange={(event) => setNewPassword(event.target.value)}
        />
      </Field>
      <Field id="cloud-new-password-confirm" label="New password again" problem={form.problem("cloud-new-password-confirm")}>
        <PasswordInput
          id="cloud-new-password-confirm"
          autoComplete="new-password"
          value={newPasswordAgain}
          onChange={(event) => setNewPasswordAgain(event.target.value)}
        />
      </Field>

      {form.error && <Notice tone="warning">{form.error}</Notice>}

      <DialogFooter>
        <Button type="button" variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="submit" disabled={form.busy}>
          {form.busy && <Loader2 className="size-4 animate-spin" />}
          {form.busy ? "Changing" : "Change password"}
        </Button>
      </DialogFooter>
    </form>
  );
}
