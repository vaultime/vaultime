// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CloudSessionContext, type CloudSessionContextValue } from "@/features/cloud/cloud-context";
import { SignUpForm } from "./AccountForms";

const PASSWORD = "correct horse battery";
const PASSPHRASE = "a long backup passphrase";

function renderForm(codeRequired: boolean) {
  const signUp = vi.fn(async (_server: string, email: string) => ({ user: { email } }));
  render(
    <CloudSessionContext.Provider value={{ signUp } as unknown as CloudSessionContextValue}>
      <SignUpForm server="https://sync.example.com" codeRequired={codeRequired} onDone={vi.fn()} />
    </CloudSessionContext.Provider>,
  );
  return signUp;
}

function fill(label: string, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

function fillAccount() {
  fill("Email", "friend@example.com");
  fill("Password", PASSWORD);
  fill("Backup passphrase", PASSPHRASE);
  fill("Backup passphrase again", PASSPHRASE);
}

describe("SignUpForm", () => {
  it("asks for the sign-up code a server needs and sends it along", async () => {
    const signUp = renderForm(true);
    fillAccount();
    fireEvent.click(screen.getByRole("button", { name: "Create account" }));
    expect(await screen.findByText("Enter the sign-up code.")).toBeTruthy();
    expect(signUp).not.toHaveBeenCalled();

    fill("Sign-up code", "river-lantern-42");
    fireEvent.click(screen.getByRole("button", { name: "Create account" }));
    await waitFor(() =>
      expect(signUp).toHaveBeenCalledWith("https://sync.example.com", "friend@example.com", PASSWORD, PASSPHRASE, "river-lantern-42"),
    );
  });

  it("shows no code field for a server without codes", async () => {
    const signUp = renderForm(false);
    expect(screen.queryByLabelText("Sign-up code")).toBeNull();
    fillAccount();
    fireEvent.click(screen.getByRole("button", { name: "Create account" }));
    await waitFor(() =>
      expect(signUp).toHaveBeenCalledWith("https://sync.example.com", "friend@example.com", PASSWORD, PASSPHRASE, null),
    );
  });
});
