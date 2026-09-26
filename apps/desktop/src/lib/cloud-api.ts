// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { MIN_BACKUP_PASSPHRASE_CHARS } from "@/lib/constants";

const configuredCloudApiBaseUrl = import.meta.env.VITE_CLOUD_API_BASE_URL?.trim();

export const CLOUD_API_BASE_URL = (
  configuredCloudApiBaseUrl && configuredCloudApiBaseUrl.length > 0
    ? configuredCloudApiBaseUrl
    : "https://vaultime.codfishcloud.de"
).replace(/\/+$/, "");

/** Prefix of invite codes, the server uses the same default. */
export const INVITE_CODE_PREFIX = "VTLINV";

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

  let response: Response;
  try {
    response = await fetch(new URL(path, CLOUD_API_BASE_URL), {
      method: options.method ?? "GET",
      headers,
      body:
        options.body === undefined ? undefined : JSON.stringify(options.body),
      signal: options.signal,
    });
  } catch {
    throw new CloudApiError(
      "Could not reach the cloud server. Check your connection.",
      0,
    );
  }

  if (response.status === 204) {
    return undefined as T;
  }

  const contentType = response.headers.get("content-type") ?? "";
  const payload = contentType.includes("application/json")
    ? await response.json()
    : await response.text();

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
    return payload.error.message;
  }

  if (typeof payload === "string" && payload.trim().length > 0) {
    return payload;
  }

  return `Cloud API request failed with status ${status}.`;
}
