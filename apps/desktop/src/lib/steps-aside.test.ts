// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import type { Game } from "@/lib/types";
import { stepsAside } from "./steps-aside";

function game(metadata: string): Game {
  return {
    id: "client",
    title: "Client",
    executable_path: null,
    install_folder: null,
    launcher_source: null,
    metadata_json: metadata,
    is_hidden: false,
    created_at: "",
    updated_at: "",
    origin_device_id: null,
  };
}

describe("stepsAside", () => {
  it("reads the switch from the game's metadata", () => {
    expect(stepsAside(game('{"steps_aside":true}'))).toBe(true);
    expect(stepsAside(game("{}"))).toBe(false);
    expect(stepsAside(game("not json"))).toBe(false);
  });
});
