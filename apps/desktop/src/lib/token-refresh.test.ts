// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { describe, expect, it, vi } from "vitest";

import { createTokenRefresher } from "./token-refresh";

type Session = { refresh_token: string; access_token: string };

function setup(start: Session | null) {
  let session = start;
  let count = 0;
  const request = vi.fn(async (token: string) => {
    count += 1;
    if (token !== session?.refresh_token) throw new Error("revoked");
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
    const { refresh, request, current } = setup({ refresh_token: "r0", access_token: "a0" });

    const [first, second] = await Promise.all([refresh("r0"), refresh("r0")]);

    expect(request).toHaveBeenCalledTimes(1);
    expect(first).toBe(second);
    expect(current()).toBe(first);
  });

  it("hands the newer session to a caller with a swapped token", async () => {
    const { refresh, request } = setup({ refresh_token: "r0", access_token: "a0" });

    const next = await refresh("r0");
    const late = await refresh("r0");

    expect(request).toHaveBeenCalledTimes(1);
    expect(late).toBe(next);
  });

  it("refreshes again once the new token is due", async () => {
    const { refresh, request } = setup({ refresh_token: "r0", access_token: "a0" });

    const next = await refresh("r0");
    const after = await refresh(next.refresh_token);

    expect(request).toHaveBeenCalledTimes(2);
    expect(after.access_token).toBe("a2");
  });

  it("lets the next caller try again after a failed refresh", async () => {
    const { refresh, request } = setup({ refresh_token: "r0", access_token: "a0" });
    request.mockRejectedValueOnce(new Error("offline"));

    await expect(refresh("r0")).rejects.toThrow("offline");
    await expect(refresh("r0")).resolves.toMatchObject({ access_token: "a1" });
  });
});
