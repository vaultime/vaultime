// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { CloudApiError } from "@/lib/cloud-api";
import { MINUTE_MS } from "@/lib/constants";
import { isExpired, parseStoredSession, shouldClearPersistedSession } from "./session-store";

describe("parseStoredSession", () => {
  it("takes a session with both tokens and a user", () => {
    const stored = { access_token: "a", refresh_token: "r", user: { id: "u" } };
    expect(parseStoredSession(JSON.stringify(stored))).toEqual(stored);
  });

  it("drops broken or incomplete sessions", () => {
    expect(parseStoredSession("not json")).toBeNull();
    expect(parseStoredSession("null")).toBeNull();
    expect(parseStoredSession(JSON.stringify({ access_token: "a", user: {} }))).toBeNull();
    expect(parseStoredSession(JSON.stringify({ access_token: "a", refresh_token: "r" }))).toBeNull();
  });
});

describe("isExpired", () => {
  it("counts a time within the margin as past and an unreadable one too", () => {
    const soon = new Date(Date.now() + MINUTE_MS).toISOString();
    expect(isExpired(soon, 0)).toBe(false);
    expect(isExpired(soon, 2 * MINUTE_MS)).toBe(true);
    expect(isExpired(new Date(Date.now() - MINUTE_MS).toISOString(), 0)).toBe(true);
    expect(isExpired("someday", 0)).toBe(true);
  });
});

describe("shouldClearPersistedSession", () => {
  it("forgets the session only when the server turned it away", () => {
    expect(shouldClearPersistedSession(new CloudApiError("expired", 401))).toBe(true);
    expect(shouldClearPersistedSession(new CloudApiError("offline", 0))).toBe(false);
    expect(shouldClearPersistedSession(new Error("other"))).toBe(false);
  });
});
