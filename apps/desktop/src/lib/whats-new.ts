// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

/** One change of a release, as the changelog words it. */
export interface ReleaseNote {
  /** The bold lead without its period, empty for a line without one. */
  title: string;
  text: string;
}

export interface ReleaseGroup {
  heading: string;
  notes: ReleaseNote[];
}

export interface Release {
  version: string;
  /** The day it came out, as the changelog dates it, null before that. */
  date: string | null;
  groups: ReleaseGroup[];
}

/** The changelog's group of fixes, which What's new only counts. */
export const FIXES_HEADING = "Fixes";

/**
 * The changelog's section for `version`, or the unreleased one for a build
 * made before the release commit. Null when neither has notes.
 */
export function releaseNotes(changelog: string, version: string): Release | null {
  const sections = changelog.replace(/\r\n/g, "\n").split(/^## /m).slice(1);
  const section =
    sections.find((each) => each.startsWith(`[${version}]`)) ?? sections.find((each) => each.startsWith("[Unreleased]"));
  if (!section) return null;
  const [head, ...lines] = section.split("\n");
  const groups: ReleaseGroup[] = [];
  for (const line of lines) {
    const group = groups.at(-1);
    const last = group?.notes.at(-1);
    if (line.startsWith("### ")) groups.push({ heading: line.slice(4).trim(), notes: [] });
    else if (line.startsWith("- ") && group) group.notes.push(note(line.slice(2)));
    else if (/^\s+\S/.test(line) && last) last.text = `${last.text} ${plain(line.trim())}`.trim();
  }
  const filled = groups.filter((group) => group.notes.length > 0);
  if (filled.length === 0) return null;
  return { version, date: /^\[[^\]]*\]\s*-\s*(\d{4}-\d{2}-\d{2})/.exec(head)?.[1] ?? null, groups: filled };
}

function note(line: string): ReleaseNote {
  const lead = /^\*\*(.+?)\*\*\s*(.*)$/.exec(line);
  if (!lead) return { title: "", text: plain(line) };
  return { title: plain(lead[1]).replace(/[.:]$/, ""), text: plain(lead[2]) };
}

/** Markdown links, emphasis and code as plain words. */
function plain(text: string): string {
  return text
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/\*([^*]+)\*/g, "$1")
    .replace(/`([^`]+)`/g, "$1");
}

/** The notes of `version` from the changelog the build carries, loaded only when asked for. */
export async function loadReleaseNotes(version: string): Promise<Release | null> {
  const { default: changelog } = await import("../../../../CHANGELOG.md?raw");
  return releaseNotes(changelog, version);
}
