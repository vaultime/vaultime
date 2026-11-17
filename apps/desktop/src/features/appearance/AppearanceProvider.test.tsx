// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useSettings } from "@/features/settings/settings-context";
import { SettingsProvider } from "@/features/settings/SettingsProvider";
import { hexToOklch } from "@/lib/color";
import {
  APPEARANCE_CACHE_KEY,
  APPEARANCE_GAME_CACHE_KEY,
  BACKGROUND_BLUR_PX,
  BACKGROUND_DIM_PERCENT,
  SETTING_KEYS,
  WINDOW_LOOK_INTERVAL_MS,
} from "@/lib/constants";
import * as api from "@/lib/tauri";
import { useAppearance, type GamesInFront } from "./appearance-context";
import { AppearanceProvider } from "./AppearanceProvider";

vi.mock("@tauri-apps/plugin-log", () => ({ warn: vi.fn(async () => {}) }));
vi.mock("@/lib/tauri", () => ({
  listSettings: vi.fn(),
  setSetting: vi.fn(async () => {}),
  getBackgroundImage: vi.fn(async () => null),
  setWindowLook: vi.fn(async () => true),
}));
vi.mock("@/lib/game-tint", async (original) => ({
  ...(await original<typeof import("@/lib/game-tint")>()),
  averageLightness: vi.fn(async () => 0.9),
}));

function Probe() {
  const { appearance } = useAppearance();
  const { reload } = useSettings();
  return (
    <button type="button" onClick={() => void reload()}>
      {appearance.accent}
    </button>
  );
}

const TEAL = { cover: "/covers/teal.jpg", colors: [{ hue: 190, chroma: 0.12 }], lightness: 0.4 };
const ORANGE = { cover: "/covers/orange.jpg", colors: [{ hue: 50, chroma: 0.15 }], lightness: 0.6 };

beforeEach(() => localStorage.clear());

/** Puts `cover` in front, unless the library has not told one, and shows what lies behind the pages. */
function BackdropProbe({ cover, lightness = 0.9 }: { cover: string | null; lightness?: number }) {
  const { backdrop, backdropDim, showGames, change } = useAppearance();
  useEffect(() => {
    if (cover) showGames({ page: { cover, colors: [], lightness }, home: { cover, colors: [], lightness } });
  }, [cover, lightness, showGames]);
  return (
    <>
      <output aria-label="Backdrop">{backdrop ?? "none"}</output>
      <output aria-label="Dim">{backdropDim}</output>
      <button type="button" onClick={() => change({ source: "none" })}>
        None
      </button>
    </>
  );
}

describe("the backdrop", () => {
  it("shows the game in front, dims light art more and goes with None", async () => {
    vi.mocked(api.listSettings).mockResolvedValueOnce([]);
    render(
      <SettingsProvider>
        <AppearanceProvider>
          <BackdropProbe cover="/covers/snowfield.jpg" />
        </AppearanceProvider>
      </SettingsProvider>,
    );
    const backdrop = await screen.findByLabelText("Backdrop");
    await waitFor(() => expect(backdrop.textContent).toBe("/covers/snowfield.jpg"));
    // 55 percent, and 20 more for art at 0.9, 0.4 above the bright line.
    await waitFor(() => expect(screen.getByLabelText("Dim").textContent).toBe("75"));
    fireEvent.click(screen.getByRole("button", { name: "None" }));
    expect(backdrop.textContent).toBe("none");
  });

  it("dims dark art more in light mode, where the text is dark", async () => {
    vi.mocked(api.listSettings).mockResolvedValueOnce([
      { key: SETTING_KEYS.appearanceMode, value: "light", updated_at: "2026-11-17T07:00:00.000Z" },
    ]);
    render(
      <SettingsProvider>
        <AppearanceProvider>
          <BackdropProbe cover="/covers/night.jpg" lightness={0.1} />
        </AppearanceProvider>
      </SettingsProvider>,
    );
    const dim = await screen.findByLabelText("Dim");
    await waitFor(() => expect(dim.textContent).toBe("75"));
  });
});

/** Dim and blur a player set for a background picture before the update. */
const EARLIER_DIM_AND_BLUR = [
  { key: SETTING_KEYS.backgroundDim, value: "82", updated_at: "2026-10-20T08:00:00.000Z" },
  { key: SETTING_KEYS.backgroundBlur, value: "8", updated_at: "2026-10-20T08:00:00.000Z" },
];

describe("an update", () => {
  it("keeps a background picture chosen before the background could follow the game", async () => {
    vi.mocked(api.listSettings).mockResolvedValueOnce(EARLIER_DIM_AND_BLUR);
    vi.mocked(api.setSetting).mockClear();
    vi.mocked(api.getBackgroundImage).mockResolvedValueOnce("data:image/png;base64,cGljdHVyZQ==");
    render(
      <SettingsProvider>
        <AppearanceProvider>
          <BackdropProbe cover="/covers/teal.jpg" />
        </AppearanceProvider>
      </SettingsProvider>,
    );
    const backdrop = await screen.findByLabelText("Backdrop");
    await waitFor(() => expect(backdrop.textContent).toBe("data:image/png;base64,cGljdHVyZQ=="));
    expect(api.setSetting).toHaveBeenCalledWith(SETTING_KEYS.backgroundSource, "picture");
    expect(api.setSetting).not.toHaveBeenCalledWith(SETTING_KEYS.backgroundDim, expect.anything());
    expect(api.setSetting).not.toHaveBeenCalledWith(SETTING_KEYS.backgroundBlur, expect.anything());
  });

  it("starts the art at the default dim and blur without a picture", async () => {
    vi.mocked(api.listSettings).mockResolvedValueOnce(EARLIER_DIM_AND_BLUR);
    vi.mocked(api.setSetting).mockClear();
    render(
      <SettingsProvider>
        <AppearanceProvider>
          <BackdropProbe cover="/covers/teal.jpg" lightness={0.4} />
        </AppearanceProvider>
      </SettingsProvider>,
    );
    const dim = await screen.findByLabelText("Dim");
    await waitFor(() => expect(dim.textContent).toBe(String(BACKGROUND_DIM_PERCENT.default)));
    expect(api.setSetting).toHaveBeenCalledWith(SETTING_KEYS.backgroundSource, "game");
    expect(api.setSetting).toHaveBeenCalledWith(SETTING_KEYS.backgroundBlur, String(BACKGROUND_BLUR_PX.default));
  });

  it("saves nothing on a fresh install, so a restore can still bring the look of a backup", async () => {
    vi.mocked(api.listSettings).mockResolvedValueOnce([]);
    vi.mocked(api.getBackgroundImage).mockClear();
    vi.mocked(api.setSetting).mockClear();
    render(
      <SettingsProvider>
        <AppearanceProvider>
          <BackdropProbe cover="/covers/teal.jpg" lightness={0.4} />
        </AppearanceProvider>
      </SettingsProvider>,
    );
    await screen.findByLabelText("Dim");
    await waitFor(() => expect(api.getBackgroundImage).toHaveBeenCalled());
    await Promise.resolve();
    expect(api.setSetting).not.toHaveBeenCalled();
  });

  it("decides nothing when the settings could not be read", async () => {
    vi.mocked(api.listSettings).mockRejectedValueOnce(new Error("locked"));
    vi.mocked(api.getBackgroundImage).mockClear();
    vi.mocked(api.getBackgroundImage).mockResolvedValueOnce("data:image/png;base64,cGljdHVyZQ==");
    vi.mocked(api.setSetting).mockClear();
    render(
      <SettingsProvider>
        <AppearanceProvider>
          <BackdropProbe cover="/covers/teal.jpg" lightness={0.4} />
        </AppearanceProvider>
      </SettingsProvider>,
    );
    await screen.findByLabelText("Dim");
    await waitFor(() => expect(api.getBackgroundImage).toHaveBeenCalled());
    await Promise.resolve();
    expect(api.setSetting).not.toHaveBeenCalled();
  });

  it("leaves dim and blur alone once a background was chosen", async () => {
    vi.mocked(api.listSettings).mockResolvedValueOnce([
      ...EARLIER_DIM_AND_BLUR,
      { key: SETTING_KEYS.backgroundSource, value: "game", updated_at: "2026-11-17T08:00:00.000Z" },
    ]);
    vi.mocked(api.setSetting).mockClear();
    render(
      <SettingsProvider>
        <AppearanceProvider>
          <BackdropProbe cover="/covers/teal.jpg" lightness={0.4} />
        </AppearanceProvider>
      </SettingsProvider>,
    );
    expect((await screen.findByLabelText("Dim")).textContent).toBe("82");
    await waitFor(() => expect(api.getBackgroundImage).toHaveBeenCalled());
    expect(api.setSetting).not.toHaveBeenCalled();
  });
});

describe("a start", () => {
  it("shows the game played last in the last run before the library is read", async () => {
    localStorage.setItem(APPEARANCE_GAME_CACHE_KEY, JSON.stringify(ORANGE));
    vi.mocked(api.listSettings).mockResolvedValueOnce([]);
    render(
      <SettingsProvider>
        <AppearanceProvider>
          <BackdropProbe cover={null} />
        </AppearanceProvider>
      </SettingsProvider>,
    );
    expect((await screen.findByLabelText("Backdrop")).textContent).toBe("/covers/orange.jpg");
  });
});

function GamesProbe({ games }: { games: GamesInFront | null }) {
  const { showGames } = useAppearance();
  useEffect(() => {
    if (games) showGames(games);
  }, [games, showGames]);
  return <p>Probe</p>;
}

/** Shows or hides the page, as the window does when it goes to the tray. */
function setPageVisible(visible: boolean) {
  Object.defineProperty(document, "visibilityState", { configurable: true, get: () => (visible ? "visible" : "hidden") });
  document.dispatchEvent(new Event("visibilitychange"));
}

/** The hue of the accent the window look last sent, of the page or of Home. */
function sentHue(look: 0 | 1 = 0) {
  return hexToOklch(vi.mocked(api.setWindowLook).mock.lastCall![look].iconAccent)!.hue;
}

describe("the window look", () => {
  it("waits for the games in front, matches the page while seen and shows the game played last in the tray", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.mocked(api.listSettings).mockResolvedValueOnce([]);
    vi.mocked(api.setWindowLook).mockClear();
    const tree = (games: GamesInFront | null) => (
      <SettingsProvider>
        <AppearanceProvider>
          <GamesProbe games={games} />
        </AppearanceProvider>
      </SettingsProvider>
    );
    try {
      const { rerender } = render(tree(null));
      await screen.findByText("Probe");
      expect(api.setWindowLook).not.toHaveBeenCalled();

      rerender(tree({ page: TEAL, home: ORANGE }));
      await waitFor(() => expect(api.setWindowLook).toHaveBeenCalledOnce());
      expect(Math.abs(sentHue() - TEAL.colors[0].hue)).toBeLessThan(5);
      // The shortcuts and the next start take Home, so browsing never rewrites them.
      expect(Math.abs(sentHue(1) - ORANGE.colors[0].hue)).toBeLessThan(5);

      act(() => setPageVisible(false));
      await vi.advanceTimersByTimeAsync(WINDOW_LOOK_INTERVAL_MS);
      await waitFor(() => expect(Math.abs(sentHue() - ORANGE.colors[0].hue)).toBeLessThan(5));
      // The next start opens Home, so only the game played last is kept for it.
      expect(JSON.parse(localStorage.getItem(APPEARANCE_GAME_CACHE_KEY)!)).toEqual(ORANGE);
      const cached = JSON.parse(localStorage.getItem(APPEARANCE_CACHE_KEY)!);
      expect(cached.tokens.dark["--violet"]).toMatch(/ 5\d(\.\d+)?\)$/);
    } finally {
      setPageVisible(true);
      vi.useRealTimers();
    }
  });
});

describe("the look after a restore", () => {
  it("follows the settings the restore brought", async () => {
    vi.mocked(api.listSettings)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([{ key: SETTING_KEYS.appearanceAccent, value: "teal", updated_at: "2026-10-20T08:00:00.000Z" }]);
    render(
      <SettingsProvider>
        <AppearanceProvider>
          <Probe />
        </AppearanceProvider>
      </SettingsProvider>,
    );

    fireEvent.click(await screen.findByRole("button", { name: "game" }));
    expect(await screen.findByRole("button", { name: "teal" })).toBeDefined();
  });
});
