// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// The same German system as the other tests, in a browser-like document.
Object.defineProperty(window.navigator, "language", { value: "de-DE", configurable: true });

afterEach(cleanup);
