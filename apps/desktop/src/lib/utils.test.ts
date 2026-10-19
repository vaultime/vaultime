// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";

import { describeError } from "./utils";

describe("errors", () => {
  it("read the message of the core's errors, of thrown errors and of plain values", () => {
    expect(describeError({ kind: "not_found", message: "it is no longer there" })).toBe("It is no longer there");
    expect(describeError(new Error("broken pipe"))).toBe("Broken pipe");
    expect(describeError("the database is locked")).toBe("The database is locked");
  });
});
