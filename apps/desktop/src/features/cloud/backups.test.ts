// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import type { CloudBackupRecord } from "@/lib/types";
import { backupSize, backupSource, formatApiHostname, formatByteSize, formatTimestamp, toIsoTimestamp } from "./backups";

function record(overrides: Partial<CloudBackupRecord> = {}): CloudBackupRecord {
  return {
    id: "backup-1",
    label: null,
    storage_key: "key",
    checksum: "sum",
    size_bytes: 2048,
    backup_created_at: "2026-10-01T10:00:00Z",
    uploaded_at: "2026-10-01T10:00:05Z",
    status: "complete",
    client_device_id: "client",
    metadata_json: null,
    ...overrides,
  };
}

describe("formatByteSize", () => {
  it("picks the unit and one decimal for small values", () => {
    expect(formatByteSize(0)).toBe("0 B");
    expect(formatByteSize(512)).toBe("512 B");
    expect(formatByteSize(4.2 * 1024 * 1024)).toBe("4.2 MiB");
    expect(formatByteSize(42 * 1024 * 1024)).toBe("42 MiB");
    expect(formatByteSize(1024 ** 5 * 3)).toBe("3072 TiB");
  });
});

describe("backups", () => {
  it("count the artwork stored apart in their size", () => {
    expect(backupSize(record())).toBe("2.0 KiB");
    const withArt = record({ metadata_json: { artwork_bytes: 1024 * 1024 } as CloudBackupRecord["metadata_json"] });
    expect(backupSize(withArt)).toBe("1.0 MiB");
  });

  it("name the PC they came from, the payload's first", () => {
    expect(backupSource(record())).toBe("client");
    const named = record({ metadata_json: { source_device_id: "desktop" } as CloudBackupRecord["metadata_json"] });
    expect(backupSource(named)).toBe("desktop");
  });
});

describe("toIsoTimestamp", () => {
  it("reads a local date and time and leaves empty or broken values out", () => {
    expect(toIsoTimestamp("")).toBeNull();
    expect(toIsoTimestamp("soon")).toBeNull();
    expect(toIsoTimestamp("2026-10-01T12:30")).toBe(new Date(2026, 9, 1, 12, 30).toISOString());
  });
});

describe("formatApiHostname", () => {
  it("shows the host of an address and copes with a bare one", () => {
    expect(formatApiHostname("https://vaultime.example.org/api")).toBe("vaultime.example.org");
    expect(formatApiHostname("https://not a url")).toBe("not a url");
  });
});

describe("formatTimestamp", () => {
  it("says when nothing is set", () => {
    expect(formatTimestamp(null)).toBe("Not set");
    expect(formatTimestamp("2026-10-01T12:30:00Z")).not.toBe("Not set");
  });
});
