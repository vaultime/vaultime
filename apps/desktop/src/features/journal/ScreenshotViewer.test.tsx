// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import * as api from "@/lib/tauri";
import type { Screenshot } from "@/lib/types";
import { ScreenshotViewer } from "./ScreenshotViewer";

vi.mock("@tauri-apps/plugin-log", () => ({ warn: vi.fn(async () => {}) }));
vi.mock("@/lib/tauri", () => ({ getSteamScreenshot: vi.fn(async () => "data:image/jpeg;base64,FULL") }));

const shots: Screenshot[] = ["a.jpg", "b.jpg"].map((file, index) => ({
  app_id: "10",
  file,
  taken_at_ms: new Date(2026, 10, 14, 15, 30 + index).getTime(),
  width: 1920,
  height: 1080,
  thumbnail: "data:image/jpeg;base64,SMALL",
}));

describe("ScreenshotViewer", () => {
  it("shows Steam's small copy until the full screenshot is read", async () => {
    render(<ScreenshotViewer title="Mournhollow" shots={shots} index={0} onIndex={() => {}} onClose={() => {}} />);
    const picture = screen.getByRole("img", { name: "Screenshot of Mournhollow" });
    expect(picture.getAttribute("src")).toBe("data:image/jpeg;base64,SMALL");
    await waitFor(() => expect(picture.getAttribute("src")).toBe("data:image/jpeg;base64,FULL"));
    expect(api.getSteamScreenshot).toHaveBeenCalledWith("10", "a.jpg");
    expect(screen.getByText("15:30")).toBeTruthy();
    expect(screen.getByText("1 of 2")).toBeTruthy();
  });

  it("steps through the screenshots with the arrow keys and the buttons", () => {
    const onIndex = vi.fn();
    render(<ScreenshotViewer title="Mournhollow" shots={shots} index={0} onIndex={onIndex} onClose={() => {}} />);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "ArrowLeft" });
    expect(onIndex).not.toHaveBeenCalled();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "ArrowRight" });
    expect(onIndex).toHaveBeenCalledWith(1);
    fireEvent.click(screen.getByRole("button", { name: "Next screenshot" }));
    expect(onIndex).toHaveBeenCalledTimes(2);
  });
});
