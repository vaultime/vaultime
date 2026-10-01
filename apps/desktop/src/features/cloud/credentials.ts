// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { MIN_BACKUP_PASSPHRASE_CHARS, MIN_CLOUD_PASSWORD_CHARS } from "@/lib/constants";

/** Characters the way the API counts them, so an emoji counts once. */
function countChars(value: string) {
  return [...value].length;
}

export const PASSWORD_HINT = `At least ${MIN_CLOUD_PASSWORD_CHARS} characters.`;
export const BACKUP_PASSPHRASE_HINT = `At least ${MIN_BACKUP_PASSPHRASE_CHARS} characters. Vaultime cannot recover it for you.`;

export function emailProblem(email: string): string | null {
  const trimmed = email.trim();
  if (!trimmed) {
    return "Enter your email address.";
  }
  if (!trimmed.includes("@")) {
    return "An email address needs an @.";
  }
  return null;
}

/** For the password of a sign in, which only has to be there. */
export function enteredPasswordProblem(password: string, what = "password"): string | null {
  return password ? null : `Enter your ${what}.`;
}

/** For a password the player chooses now. */
export function newPasswordProblem(password: string): string | null {
  if (!password) {
    return "Choose a password.";
  }
  const count = countChars(password);
  if (count < MIN_CLOUD_PASSWORD_CHARS) {
    return `Use at least ${MIN_CLOUD_PASSWORD_CHARS} characters. This one has ${count}.`;
  }
  return null;
}

/** Spaces at either end do not count, the app trims them before deriving the key. */
export function backupPassphraseProblem(passphrase: string): string | null {
  const count = countChars(passphrase.trim());
  if (count === 0) {
    return "Choose a backup passphrase.";
  }
  if (count < MIN_BACKUP_PASSPHRASE_CHARS) {
    return `Use at least ${MIN_BACKUP_PASSPHRASE_CHARS} characters. This one has ${count}.`;
  }
  return null;
}

export function inviteCodeProblem(code: string): string | null {
  return code.trim() ? null : "Enter the invite code you received.";
}

/** For the second entry of a password or passphrase, `what` names the first. */
export function repeatProblem(first: string, again: string, what: string): string | null {
  if (!again) {
    return `Enter the ${what} again.`;
  }
  if (again !== first) {
    return `This does not match the ${what} above.`;
  }
  return null;
}

/** The id of the first field with a problem, to move the focus there. */
export function firstProblem(problems: Record<string, string | null>): string | null {
  return Object.entries(problems).find(([, problem]) => problem)?.[0] ?? null;
}
