// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useCallback, useEffect, useState } from "react";
import { ACTIVE_POLL_MS } from "@/lib/constants";
import type { Session } from "@/lib/types";
import * as api from "@/lib/tauri";

export function useSessions() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Only the first load shows the spinner. Later refreshes swap data in place.
  const refresh = useCallback(
    () =>
      api
        .listSessions()
        .then((list) => {
          setSessions(list);
          setError(null);
        })
        .catch((e: unknown) => setError(String(e)))
        .finally(() => setLoading(false)),
    [],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { sessions, loading, error, refresh };
}

function sameSessions(a: Session[], b: Session[]): boolean {
  return a.length === b.length && JSON.stringify(a) === JSON.stringify(b);
}

/** Polls open sessions. The array keeps its identity while nothing changes. */
export function useActiveSessions(pollIntervalMs = ACTIVE_POLL_MS) {
  const [activeSessions, setActiveSessions] = useState<Session[]>([]);

  useEffect(() => {
    let cancelled = false;

    function poll() {
      api
        .getActiveSessions()
        .then((active) => {
          if (!cancelled) {
            setActiveSessions((current) =>
              sameSessions(current, active) ? current : active,
            );
          }
        })
        .catch(() => {
          // Polling errors are transient. The next tick retries.
        });
    }

    poll();
    const id = setInterval(poll, pollIntervalMs);

    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [pollIntervalMs]);

  return { activeSessions };
}
