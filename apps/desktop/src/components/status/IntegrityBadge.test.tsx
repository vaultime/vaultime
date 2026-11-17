// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { IntegrityBadge } from "./IntegrityBadge";

describe("the trust label", () => {
  it("says Checking, never Local, until the chain is checked", () => {
    render(
      <>
        <IntegrityBadge status="local" checking />
        <IntegrityBadge status="recovered" checking />
        <IntegrityBadge status="local" />
      </>,
    );
    expect(screen.getAllByText("Checking")).toHaveLength(2);
    expect(screen.getByText("Local")).toBeTruthy();
  });

  it("says Suspicious right away, as a check cannot take it back", () => {
    render(<IntegrityBadge status="suspicious" checking />);
    expect(screen.getByText("Suspicious")).toBeTruthy();
    expect(screen.queryByText("Checking")).toBeNull();
  });
});
