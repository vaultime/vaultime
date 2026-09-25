// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Routes, Route, Navigate } from "react-router";
import { AppLayout } from "@/components/layout/AppLayout";
import { LibraryPage } from "@/features/library/LibraryPage";
import { SessionsPage } from "@/features/sessions/SessionsPage";
import { CloudPage } from "@/features/cloud/CloudPage";
import { SettingsPage } from "@/features/settings/SettingsPage";

export function App() {
  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route index element={<Navigate to="/library" replace />} />
        <Route path="library" element={<LibraryPage />} />
        <Route path="sessions" element={<SessionsPage />} />
        <Route path="cloud" element={<CloudPage />} />
        <Route path="settings" element={<SettingsPage />} />
      </Route>
    </Routes>
  );
}
