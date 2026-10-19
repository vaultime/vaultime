// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { CLOUD_REQUEST_TIMEOUT_MS, MIN_BACKUP_PASSPHRASE_CHARS, VAULTIME_URL } from "@/lib/constants";
import { sentence } from "@/lib/words";

const configuredCloudApiBaseUrl = import.meta.env.VITE_CLOUD_API_BASE_URL?.trim();

export const CLOUD_API_BASE_URL = (
  configuredCloudApiBaseUrl && configuredCloudApiBaseUrl.length > 0
    ? configuredCloudApiBaseUrl
    : VAULTIME_URL
).replace(/\/+$/, "");

/** Shown when the server cannot be reached. */
export const CLOUD_UNREACHABLE = "Could not reach the cloud server. Check your connection.";
/** Shown when the server took longer than `CLOUD_REQUEST_TIMEOUT_MS` to answer. */
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

interface RequestOptions {
  method?: "GET" | "POST" | "DELETE";
  body?: unknown;
  accessToken?: string;
  signal?: AbortSignal;
}

export async function cloudGetJson<T>(
  path: string,
  accessToken?: string,
  signal?: AbortSignal,
): Promise<T> {
  return cloudRequest<T>(path, {
    method: "GET",
    accessToken,
    signal,
  });
}

export async function cloudPostJson<T>(
  path: string,
  body: unknown,
  accessToken?: string,
  signal?: AbortSignal,
): Promise<T> {
  return cloudRequest<T>(path, {
    method: "POST",
    body,
    accessToken,
    signal,
  });
}

export async function cloudDelete(
  path: string,
  accessToken?: string,
  signal?: AbortSignal,
): Promise<void> {
  await cloudRequest<void>(path, {
    method: "DELETE",
    accessToken,
    signal,
  });
}

async function cloudRequest<T>(
  path: string,
  options: RequestOptions,
): Promise<T> {
  const headers = new Headers();
  if (options.body !== undefined) {
    headers.set("Content-Type", "application/json");
  }
  if (options.accessToken) {
    headers.set("Authorization", `Bearer ${options.accessToken}`);
  }

  // A server that stops answering must not hold up the page or the daily
  // backup, so every request, its body included, ends after the timeout.
  const controller = new AbortController();
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    controller.abort();
  }, CLOUD_REQUEST_TIMEOUT_MS);
  const abort = () => controller.abort(options.signal?.reason);
  if (options.signal?.aborted) abort();
  options.signal?.addEventListener("abort", abort, { once: true });
  const failed = () => new CloudApiError(timedOut ? CLOUD_TIMED_OUT : CLOUD_UNREACHABLE, 0);

  let response: Response;
  let payload: unknown;
  try {
    try {
      response = await fetch(new URL(path, CLOUD_API_BASE_URL), {
        method: options.method ?? "GET",
        headers,
        body:
          options.body === undefined ? undefined : JSON.stringify(options.body),
        signal: controller.signal,
      });
    } catch {
      throw failed();
    }

    if (response.status === 204) {
      return undefined as T;
    }

    const contentType = response.headers.get("content-type") ?? "";
    try {
      payload = contentType.includes("application/json")
        ? await response.json()
        : await response.text();
    } catch (error) {
      if (controller.signal.aborted) throw failed();
      throw error;
    }
  } finally {
    clearTimeout(timer);
    options.signal?.removeEventListener("abort", abort);
  }

  if (!response.ok) {
    throw new CloudApiError(
      extractErrorMessage(payload, response.status),
      response.status,
    );
  }

  return payload as T;
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

  if (typeof payload === "string" && payload.trim().length > 0) {
    return payload;
  }

  return `The cloud server answered with error ${status}.`;
}
