// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Screenshot } from "@/lib/types";
import { ScreenshotStrip } from "./ScreenshotStrip";

function shot(file: string, thumbnail: string | null): Screenshot {
  return {
    app_id: "10",
    file,
    taken_at_ms: new Date(2026, 10, 14, 15, 30).getTime(),
    width: 1920,
    height: 1080,
    thumbnail,
  };
}

describe("ScreenshotStrip", () => {
  it("shows the screenshots in small with when each was taken and counts the rest", () => {
    const onOpen = vi.fn();
    render(
      <ScreenshotStrip
        screenshots={{
          session_id: "s",
          total: 4,
          shots: [shot("a.jpg", "data:image/jpeg;base64,AA"), shot("b.jpg", null)],
        }}
        onOpen={onOpen}
      />,
    );
    const shots = screen.getAllByRole("button", { name: "Screenshot from 15:30" });
    expect(shots).toHaveLength(2);
    expect(shots[0].querySelector("img")?.getAttribute("src")).toBe("data:image/jpeg;base64,AA");
    fireEvent.click(shots[1]);
    expect(onOpen).toHaveBeenCalledWith(1);
    expect(screen.getByText("and two more")).toBeTruthy();
  });
});
