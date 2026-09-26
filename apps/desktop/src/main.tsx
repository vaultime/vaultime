// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router";
import { CloudSessionProvider } from "@/features/cloud/CloudSessionProvider";
import { LibraryProvider } from "@/features/library/LibraryProvider";
import { App } from "@/App";
import "./index.css";

document.documentElement.classList.add("dark");

if (import.meta.env.VITE_MOCK_IPC === "1") {
  await import("./dev/mock-ipc");
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <CloudSessionProvider>
        <LibraryProvider>
          <App />
        </LibraryProvider>
      </CloudSessionProvider>
    </BrowserRouter>
  </StrictMode>,
);
