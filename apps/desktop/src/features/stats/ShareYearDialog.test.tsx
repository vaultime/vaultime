// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppearanceContext } from "@/features/appearance/appearance-context";
import { LibraryContext } from "@/features/library/library-context";
import type { YearStats } from "@/lib/stats";
import { getCoverDataUrls, saveShareImage } from "@/lib/tauri";
import { ShareYearDialog } from "./ShareYearDialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(async () => "C:/cards/year.png") }));
vi.mock("@/lib/tauri", () => ({
  getThisPc: vi.fn(async () => ({ name: "Desk" })),
  getCoverDataUrls: vi.fn(),
  saveShareImage: vi.fn(async () => {}),
}));
vi.mock("./share-card-draw", () => ({ drawShareCard: vi.fn(async () => {}) }));
// A card naming two games with covers and one with a typeset cover.
vi.mock("@/lib/share-card", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/share-card")>()),
  buildShareCard: () => ({
    year: 2026,
    activeMs: 0,
    daysPlayed: 0,
    games: [
      { gameId: "hades", cover: "art://hades" },
      { gameId: "typeset", cover: null },
      { gameId: "celeste", cover: "art://celeste" },
    ],
  }),
  shareChoices: () => [],
}));

function show() {
  render(
    <AppearanceContext.Provider
      value={{ appearance: { ground: "graphite" }, mode: "dark", accentHues: [], accent: "#d1d1d1" } as never}
    >
      <LibraryContext.Provider value={{ summaries: [], covers: {} } as never}>
        <ShareYearDialog open onOpenChange={() => {}} stats={{ year: 2026 } as YearStats} now={new Date(2026, 10, 18)} />
      </LibraryContext.Provider>
    </AppearanceContext.Provider>,
  );
}

/** Saves that failed before the one waited for, which never reach the save call. */
let failures = 0;

async function save(times: number) {
  const button = await screen.findByRole("button", { name: /Save as PNG/ });
  fireEvent.click(button);
  await waitFor(() => expect(vi.mocked(saveShareImage).mock.calls.length + failures).toBe(times));
}

beforeEach(() => {
  failures = 0;
  vi.mocked(getCoverDataUrls).mockReset();
  vi.mocked(getCoverDataUrls).mockImplementation(async (ids: string[]) =>
    Object.fromEntries(ids.map((id) => [id, `data:image/jpeg;base64,${id}`])),
  );
  vi.mocked(saveShareImage).mockClear();
  HTMLCanvasElement.prototype.toBlob = function (done: BlobCallback) {
    done(new Blob(["card"], { type: "image/png" }));
  };
});

describe("saving the card of the year", () => {
  it("reads each shown cover once while the dialog is open, and no typeset one", async () => {
    show();
    await save(1);
    await save(2);
    expect(getCoverDataUrls).toHaveBeenCalledTimes(1);
    expect(getCoverDataUrls).toHaveBeenCalledWith(["hades", "celeste"]);
  });

  it("reads the covers again after a read that failed", async () => {
    vi.mocked(getCoverDataUrls).mockRejectedValueOnce(new Error("busy"));
    show();
    fireEvent.click(await screen.findByRole("button", { name: /Save as PNG/ }));
    await screen.findByText("Busy");
    failures = 1;
    await save(2);
    expect(getCoverDataUrls).toHaveBeenCalledTimes(2);
    expect(vi.mocked(getCoverDataUrls).mock.calls[1][0]).toEqual(["hades", "celeste"]);
  });
});
