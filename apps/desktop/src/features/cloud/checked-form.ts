// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState, type FormEvent } from "react";
import { firstProblem } from "@/features/cloud/credentials";
import { describeError } from "@/lib/utils";

/**
 * Form state for fields keyed by their element id. Problems show from the
 * first submit on and clear while the player fixes them. Errors of the
 * action itself, like a wrong password, are kept for the form to show.
 */
export function useCheckedForm<Id extends string>(problems: Record<Id, string | null>) {
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(event: FormEvent<HTMLFormElement>, action: () => Promise<void>) {
    event.preventDefault();
    setChecked(true);
    setError(null);
    const first = firstProblem(problems);
    if (first) {
      document.getElementById(first)?.focus();
      return;
    }
    setBusy(true);
    try {
      await action();
    } catch (actionError) {
      setError(describeError(actionError));
    } finally {
      setBusy(false);
    }
  }

  return {
    problem: (id: Id) => (checked ? problems[id] : null),
    busy,
    error,
    submit,
  };
}
