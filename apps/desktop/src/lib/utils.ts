import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/** The message of an error, or the value itself as text. */
export function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
