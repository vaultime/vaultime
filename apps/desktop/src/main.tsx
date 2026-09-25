// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router";
import { CloudSessionProvider } from "@/features/cloud/CloudSessionProvider";
import { TooltipProvider } from "@/components/ui/tooltip";
import { App } from "@/App";
import "./index.css";

document.documentElement.classList.add("dark");

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <CloudSessionProvider>
        <TooltipProvider>
          <App />
        </TooltipProvider>
      </CloudSessionProvider>
    </BrowserRouter>
  </StrictMode>,
);
