// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { at, session } from "@/test/sessions";
import type { Device } from "@/lib/types";
import { isMergedPc, pcName, recordedElsewhere } from "./devices";

function device(id: string, name: string | null, mergedAt: string | null = null): Device {
  return { id, platform: "windows", app_version: "0.4.0", key_id: null, registered_at: "", name, merged_at: mergedAt };
}

const devices = [device("desktop", "Desktop"), device("laptop", "Laptop", "2026-10-03T12:00:00Z"), device("old", null)];

describe("devices", () => {
  it("names a PC, or falls back to its id", () => {
    expect(pcName(devices, "laptop")).toBe("Laptop");
    expect(pcName(devices, "old")).toBe("old");
    expect(pcName(devices, "unknown")).toBe("unknown");
  });

  it("names the PC of a session only when it is another one", () => {
    const here = session(at(2026, 10, 2, 18), 60, { device_id: "desktop" });
    const there = session(at(2026, 10, 2, 18), 60, { device_id: "laptop" });
    expect(recordedElsewhere(here, devices, "desktop")).toBeNull();
    expect(recordedElsewhere(there, devices, "desktop")).toBe("Laptop");
    expect(recordedElsewhere(there, devices, null)).toBeNull();
  });

  it("knows which PCs came by merge", () => {
    expect(isMergedPc(devices, "laptop")).toBe(true);
    expect(isMergedPc(devices, "old")).toBe(false);
  });
});
