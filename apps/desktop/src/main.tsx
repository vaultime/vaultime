// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router";
import { AppearanceProvider } from "@/features/appearance/AppearanceProvider";
import { AwayTimerProvider } from "@/features/away/AwayTimerProvider";
import { CloudSessionProvider } from "@/features/cloud/CloudSessionProvider";
import { LibraryProvider } from "@/features/library/LibraryProvider";
import { SettingsProvider } from "@/features/settings/SettingsProvider";
import { SyncFolderProvider } from "@/features/settings/SyncFolderProvider";
import { UpdateProvider } from "@/features/updates/UpdateProvider";
import { App } from "@/App";
import { stillBlur } from "@/lib/still-blur";
import "./index.css";

if (import.meta.env.VITE_MOCK_IPC === "1") {
  await import("./dev/mock-ipc");
}
// Small controls take a see-through fill instead of a live blur behind them.
if (stillBlur()) document.documentElement.dataset.blur = "still";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <SettingsProvider>
        <AppearanceProvider>
          <CloudSessionProvider>
            <LibraryProvider>
              <SyncFolderProvider>
                <AwayTimerProvider>
                  <UpdateProvider>
                    <App />
                  </UpdateProvider>
                </AwayTimerProvider>
              </SyncFolderProvider>
            </LibraryProvider>
          </CloudSessionProvider>
        </AppearanceProvider>
      </SettingsProvider>
    </BrowserRouter>
  </StrictMode>,
);
