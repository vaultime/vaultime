// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { MIN_BACKUP_PASSPHRASE_CHARS } from "@/lib/constants";
import { cloudRequest } from "@/lib/tauri";
import { sentence } from "@/lib/words";

/** Shown when the server cannot be reached. */
export const CLOUD_UNREACHABLE = "Could not reach the cloud server. Check your connection.";
/** Shown when the server did not answer in time. */
export const CLOUD_TIMED_OUT = "The cloud server did not answer in time. Try again later.";

/** Shown when a backup passphrase is shorter than the minimum. */
export const BACKUP_PASSPHRASE_TOO_SHORT = `The backup passphrase needs at least ${MIN_BACKUP_PASSPHRASE_CHARS} characters.`;

export class CloudApiError extends Error {
  public readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "CloudApiError";
    this.status = status;
  }
}

export async function cloudGetJson<T>(server: string, path: string, accessToken?: string): Promise<T> {
  return send<T>(server, "GET", path, undefined, accessToken);
}

export async function cloudPostJson<T>(server: string, path: string, body: unknown, accessToken?: string): Promise<T> {
  return send<T>(server, "POST", path, body, accessToken);
}

export async function cloudDelete(server: string, path: string, accessToken?: string): Promise<void> {
  await send<void>(server, "DELETE", path, undefined, accessToken);
}

// The core sends every request, so the page needs no network access of its
// own, and a server that stops answering runs into the core's timeout.
async function send<T>(
  server: string,
  method: "GET" | "POST" | "DELETE",
  path: string,
  body: unknown,
  accessToken: string | undefined,
): Promise<T> {
  const reply = await cloudRequest(server, method, path, body, accessToken);
  if (reply.kind === "timed_out") throw new CloudApiError(CLOUD_TIMED_OUT, 0);
  if (reply.kind === "unreachable") throw new CloudApiError(CLOUD_UNREACHABLE, 0);
  if (reply.status < 200 || reply.status >= 300) {
    throw new CloudApiError(extractErrorMessage(reply.body, reply.status), reply.status);
  }
  return (reply.status === 204 ? undefined : reply.body) as T;
}

function extractErrorMessage(payload: unknown, status: number): string {
  if (
    typeof payload === "object" &&
    payload !== null &&
    "error" in payload &&
    typeof payload.error === "object" &&
    payload.error !== null &&
    "message" in payload.error &&
    typeof payload.error.message === "string"
  ) {
    // The server writes its messages in lowercase without a full stop.
    return sentence(payload.error.message);
  }

  // A proxy in front of a server that is down answers with a page of its own.
  if (typeof payload === "string" && payload.trim().length > 0 && !payload.trim().startsWith("<")) {
    return payload;
  }

  return `The cloud server answered with error ${status}.`;
}
