// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useCallback, useEffect, useState } from "react";
import type { Game } from "@/lib/types";
import * as api from "@/lib/tauri";

export function useGames() {
  const [games, setGames] = useState<Game[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Only the first load shows the spinner. Later refreshes swap data in place.
  const refresh = useCallback(
    () =>
      api
        .listGames()
        .then((list) => {
          setGames(list);
          setError(null);
        })
        .catch((e: unknown) => setError(String(e)))
        .finally(() => setLoading(false)),
    [],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { games, loading, error, refresh };
}
