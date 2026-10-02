// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router";
import { AppearanceProvider } from "@/features/appearance/AppearanceProvider";
import { CloudSessionProvider } from "@/features/cloud/CloudSessionProvider";
import { LibraryProvider } from "@/features/library/LibraryProvider";
import { App } from "@/App";
import "./index.css";

if (import.meta.env.VITE_MOCK_IPC === "1") {
  await import("./dev/mock-ipc");
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <AppearanceProvider>
        <CloudSessionProvider>
          <LibraryProvider>
            <App />
          </LibraryProvider>
        </CloudSessionProvider>
      </AppearanceProvider>
    </BrowserRouter>
  </StrictMode>,
);
