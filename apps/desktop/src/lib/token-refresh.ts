// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Wraps a refresh request for single use refresh tokens. Callers that refresh
 * at the same time share one request, and a caller that holds a token which
 * was already swapped gets the newer session instead of spending a dead token,
 * which the server would answer as an expired session.
 */
export function createTokenRefresher<Session extends { refresh_token: string }>(
  request: (stale: Session) => Promise<Session>,
  current: () => Session | null,
  swap: (next: Session) => void,
): (stale: Session) => Promise<Session> {
  let pending: { token: string; promise: Promise<Session> } | null = null;

  return (stale) => {
    const refreshToken = stale.refresh_token;
    const latest = current();
    if (latest && latest.refresh_token !== refreshToken) {
      return Promise.resolve(latest);
    }
    if (pending?.token === refreshToken) {
      return pending.promise;
    }
    const promise = request(stale)
      .then((next) => {
        if (current()?.refresh_token === refreshToken) {
          swap(next);
        }
        return next;
      })
      .finally(() => {
        if (pending?.promise === promise) {
          pending = null;
        }
      });
    pending = { token: refreshToken, promise };
    return promise;
  };
}
