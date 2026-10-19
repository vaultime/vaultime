// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { afterEach, describe, expect, it, vi } from "vitest";

import { CLOUD_TIMED_OUT, CLOUD_UNREACHABLE, cloudGetJson } from "./cloud-api";
import { CLOUD_REQUEST_TIMEOUT_MS } from "./constants";

/** A fetch that never answers until its signal aborts. */
function hangingFetch() {
  return vi.fn(
    (_url: URL, init: RequestInit) =>
      new Promise<Response>((_resolve, reject) => {
        init.signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
      }),
  );
}

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("cloud requests", () => {
  it("give up on a server that never answers", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("fetch", hangingFetch());
    const request = cloudGetJson("/v1/storage", "token");
    const settled = expect(request).rejects.toMatchObject({ name: "CloudApiError", message: CLOUD_TIMED_OUT, status: 0 });
    await vi.advanceTimersByTimeAsync(CLOUD_REQUEST_TIMEOUT_MS);
    await settled;
  });

  it("give up on an answer whose body never ends", async () => {
    vi.useFakeTimers();
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url: URL, init: RequestInit) => {
        // As in a browser, aborting the request breaks off its body.
        const body = new ReadableStream({
          start(stream) {
            init.signal?.addEventListener("abort", () => stream.error(new DOMException("aborted", "AbortError")));
          },
        });
        return new Response(body, { headers: { "content-type": "application/json" } });
      }),
    );
    const request = cloudGetJson("/v1/backups", "token");
    const settled = expect(request).rejects.toMatchObject({ name: "CloudApiError", message: CLOUD_TIMED_OUT, status: 0 });
    await vi.advanceTimersByTimeAsync(CLOUD_REQUEST_TIMEOUT_MS);
    await settled;
  });

  it("say when the server cannot be reached", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("Failed to fetch")));
    await expect(cloudGetJson("/v1/storage")).rejects.toMatchObject({ name: "CloudApiError", message: CLOUD_UNREACHABLE, status: 0 });
  });

  it("pass on the server's message and status", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        Response.json({ error: { code: "full", message: "storage is full" } }, { status: 413 }),
      ),
    );
    await expect(cloudGetJson("/v1/storage")).rejects.toMatchObject({ name: "CloudApiError", message: "Storage is full.", status: 413 });
  });

  it("stop with the caller's signal", async () => {
    vi.stubGlobal("fetch", hangingFetch());
    const caller = new AbortController();
    const request = cloudGetJson("/v1/storage", undefined, caller.signal);
    caller.abort();
    await expect(request).rejects.toMatchObject({ name: "CloudApiError", message: CLOUD_UNREACHABLE, status: 0 });
  });
});
