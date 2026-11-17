// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { beforeAll, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri", () => ({ cloudRequest: vi.fn() }));

const SERVER = "https://vaultime.example.com";

describe("cloud requests", () => {
  // These tests share a module cache with the files before them in the
  // worker, where cloud-api may already hold another mock of the core.
  let cloudRequest: typeof import("@/lib/tauri").cloudRequest;
  let api: typeof import("./cloud-api");
  beforeAll(async () => {
    vi.resetModules();
    ({ cloudRequest } = await import("@/lib/tauri"));
    api = await import("./cloud-api");
  });

  it("go through the core with the server, the body and the token", async () => {
    vi.mocked(cloudRequest).mockResolvedValueOnce({ kind: "answer", status: 200, body: { ok: true } });
    await expect(api.cloudPostJson(SERVER, "/v1/auth/login", { email: "a@b.c" }, "token")).resolves.toEqual({ ok: true });
    expect(cloudRequest).toHaveBeenCalledWith(SERVER, "POST", "/v1/auth/login", { email: "a@b.c" }, "token");
  });

  it("say when the server did not answer in time", async () => {
    vi.mocked(cloudRequest).mockResolvedValueOnce({ kind: "timed_out" });
    await expect(api.cloudGetJson(SERVER, "/v1/storage", "token")).rejects.toMatchObject({
      name: "CloudApiError",
      message: api.CLOUD_TIMED_OUT,
      status: 0,
    });
  });

  it("say when the server cannot be reached", async () => {
    vi.mocked(cloudRequest).mockResolvedValueOnce({ kind: "unreachable" });
    await expect(api.cloudGetJson(SERVER, "/v1/storage")).rejects.toMatchObject({
      name: "CloudApiError",
      message: api.CLOUD_UNREACHABLE,
      status: 0,
    });
  });

  it("pass on the server's message and status", async () => {
    vi.mocked(cloudRequest).mockResolvedValueOnce({
      kind: "answer",
      status: 413,
      body: { error: { code: "full", message: "storage is full" } },
    });
    await expect(api.cloudGetJson(SERVER, "/v1/storage")).rejects.toMatchObject({ name: "CloudApiError", message: "Storage is full.", status: 413 });
  });

  it("leave out the page a proxy answers with", async () => {
    vi.mocked(cloudRequest).mockResolvedValueOnce({ kind: "answer", status: 502, body: "<html><body>Bad gateway</body></html>" });
    await expect(api.cloudGetJson(SERVER, "/v1/storage")).rejects.toMatchObject({ message: "The cloud server answered with error 502.", status: 502 });
  });

  it("take an empty answer as nothing", async () => {
    vi.mocked(cloudRequest).mockResolvedValueOnce({ kind: "answer", status: 204, body: "" });
    await expect(api.cloudDelete(SERVER, "/v1/devices/1", "token")).resolves.toBeUndefined();
  });
});
