// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import {
  backupPassphraseProblem,
  emailProblem,
  enteredPasswordProblem,
  firstProblem,
  inviteCodeProblem,
  newPasswordProblem,
  repeatProblem,
} from "@/features/cloud/credentials";

describe("emailProblem", () => {
  it("asks for an address that is missing or has no @", () => {
    expect(emailProblem("   ")).toBe("Enter your email address.");
    expect(emailProblem("player.example.com")).toBe("An email address needs an @.");
    expect(emailProblem(" player@example.com ")).toBeNull();
  });
});

describe("newPasswordProblem", () => {
  it("says how many characters are missing", () => {
    expect(newPasswordProblem("")).toBe("Choose a password.");
    expect(newPasswordProblem("short")).toBe("Use at least 10 characters. This one has 5.");
    expect(newPasswordProblem("long enough")).toBeNull();
  });

  it("counts characters like the server, an emoji once", () => {
    expect(newPasswordProblem("🎮🎮🎮🎮🎮")).toBe("Use at least 10 characters. This one has 5.");
    expect(newPasswordProblem("🎮".repeat(10))).toBeNull();
  });

  it("keeps spaces, the server does not trim passwords", () => {
    expect(newPasswordProblem("     abcde")).toBeNull();
  });
});

describe("backupPassphraseProblem", () => {
  it("ignores spaces at the ends, the key is derived from the trimmed passphrase", () => {
    expect(backupPassphraseProblem("   ")).toBe("Choose a backup passphrase.");
    expect(backupPassphraseProblem("  eleven char  ")).toBe("Use at least 12 characters. This one has 11.");
    expect(backupPassphraseProblem("twelve chars")).toBeNull();
  });
});

describe("repeatProblem", () => {
  it("asks for the second entry and compares it exactly", () => {
    expect(repeatProblem("secret value", "", "new password")).toBe("Enter the new password again.");
    expect(repeatProblem("secret value", "secret value ", "new password")).toBe(
      "This does not match the new password above.",
    );
    expect(repeatProblem("secret value", "secret value", "new password")).toBeNull();
  });
});

describe("single checks", () => {
  it("only need something entered", () => {
    expect(enteredPasswordProblem("")).toBe("Enter your password.");
    expect(enteredPasswordProblem("", "current password")).toBe("Enter your current password.");
    expect(enteredPasswordProblem("x")).toBeNull();
    expect(inviteCodeProblem(" ")).toBe("Enter the invite code you received.");
    expect(inviteCodeProblem("VTLINV-ABCD")).toBeNull();
  });
});

describe("firstProblem", () => {
  it("names the first field in order that has a problem", () => {
    expect(firstProblem({ email: null, password: "Enter your password.", again: "Enter it again." })).toBe("password");
    expect(firstProblem({ email: null, password: null })).toBeNull();
  });
});
