// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NoticesDialog } from "./NoticesDialog";

describe("the third-party notices", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("shows what the build wrote", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response("Mona Sans 5.3.0, OFL-1.1", { status: 200 })));
    render(<NoticesDialog onClose={() => {}} />);
    expect(await screen.findByText("Mona Sans 5.3.0, OFL-1.1")).toBeDefined();
  });

  it("says so when the build carries none", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response("", { status: 404 })));
    render(<NoticesDialog onClose={() => {}} />);
    expect(await screen.findByText("This build carries no notices. The installers do.")).toBeDefined();
  });
});
