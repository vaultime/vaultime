// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it, vi } from "vitest";

import { createTokenRefresher } from "./token-refresh";

type Session = { refresh_token: string; access_token: string };

const START: Session = { refresh_token: "r0", access_token: "a0" };

function setup(start: Session | null) {
  let session = start;
  let count = 0;
  const request = vi.fn(async (stale: Session) => {
    count += 1;
    if (stale.refresh_token !== session?.refresh_token) throw new Error("revoked");
    return { refresh_token: `r${count}`, access_token: `a${count}` };
  });
  const refresh = createTokenRefresher<Session>(
    request,
    () => session,
    (next) => {
      session = next;
    },
  );
  return { refresh, request, current: () => session };
}

describe("createTokenRefresher", () => {
  it("shares one request between callers that refresh at the same time", async () => {
    const { refresh, request, current } = setup(START);

    const [first, second] = await Promise.all([refresh(START), refresh(START)]);

    expect(request).toHaveBeenCalledTimes(1);
    expect(first).toBe(second);
    expect(current()).toBe(first);
  });

  it("hands the newer session to a caller with a swapped token", async () => {
    const { refresh, request } = setup(START);

    const next = await refresh(START);
    const late = await refresh(START);

    expect(request).toHaveBeenCalledTimes(1);
    expect(late).toBe(next);
  });

  it("refreshes again once the new token is due", async () => {
    const { refresh, request } = setup(START);

    const next = await refresh(START);
    const after = await refresh(next);

    expect(request).toHaveBeenCalledTimes(2);
    expect(after.access_token).toBe("a2");
  });

  it("lets the next caller try again after a failed refresh", async () => {
    const { refresh, request } = setup(START);
    request.mockRejectedValueOnce(new Error("offline"));

    await expect(refresh(START)).rejects.toThrow("offline");
    await expect(refresh(START)).resolves.toMatchObject({ access_token: "a1" });
  });
});
