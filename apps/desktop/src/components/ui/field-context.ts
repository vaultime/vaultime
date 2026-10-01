// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext } from "react";

export interface FieldControl {
  messageId?: string;
  invalid: boolean;
}

export const FieldContext = createContext<FieldControl | null>(null);

/** Lets an input inside a Field point to its hint or problem. */
export function useFieldControl() {
  return useContext(FieldContext);
}
