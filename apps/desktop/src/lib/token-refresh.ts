// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

/**
 * Wraps a refresh request for single use refresh tokens. Callers that refresh
 * at the same time share one request, and a caller that holds a token which
 * was already swapped gets the newer session instead of spending a dead token,
 * which the server would answer as an expired session.
 */
export function createTokenRefresher<Session extends { refresh_token: string }>(
  request: (refreshToken: string) => Promise<Session>,
  current: () => Session | null,
  swap: (next: Session) => void,
): (refreshToken: string) => Promise<Session> {
  let pending: { token: string; promise: Promise<Session> } | null = null;

  return (refreshToken) => {
    const latest = current();
    if (latest && latest.refresh_token !== refreshToken) {
      return Promise.resolve(latest);
    }
    if (pending?.token === refreshToken) {
      return pending.promise;
    }
    const promise = request(refreshToken)
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
