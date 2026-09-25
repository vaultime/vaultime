// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useCallback, useEffect, useState } from "react";
import type { Session } from "@/lib/types";
import * as api from "@/lib/tauri";

/** Manages the session list state. */
export function useSessions() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const list = await api.listSessions();
      setSessions(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return { sessions, loading, error, refresh };
}

/** Tracks currently active (open) sessions, polling periodically. */
export function useActiveSessions(pollIntervalMs = 5_000) {
  const [activeSessions, setActiveSessions] = useState<Session[]>([]);

  useEffect(() => {
    let cancelled = false;

    async function refresh() {
      try {
        const active = await api.getActiveSessions();
        if (!cancelled) {
          setActiveSessions(active);
        }
      } catch {
        // Silently ignore polling errors to avoid UI noise.
      }
    }

    void refresh();
    const id = setInterval(() => {
      void refresh();
    }, pollIntervalMs);

    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [pollIntervalMs]);

  const refresh = useCallback(async () => {
    try {
      const active = await api.getActiveSessions();
      setActiveSessions(active);
    } catch {
      // Silently ignore polling errors to avoid UI noise.
    }
  }, []);

  return { activeSessions, refresh };
}
