// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Routes, Route, Navigate, Outlet } from "react-router";
import { AppLayout } from "@/components/layout/AppLayout";
import { LibraryPage } from "@/features/library/LibraryPage";
import { JournalPage } from "@/features/journal/JournalPage";
import { CloudPage } from "@/features/cloud/CloudPage";
import { SettingsPage } from "@/features/settings/SettingsPage";
import { GameDetailsPage } from "@/features/game-details";

export function App() {
  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route index element={<Navigate to="/library" replace />} />
        <Route path="library" element={<LibraryPage />} />
        <Route path="library/:gameId" element={<GameDetailsPage />} />
        <Route path="journal" element={<JournalPage />} />
        <Route path="sessions" element={<Navigate to="/journal" replace />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route element={<PaddedPage />}>
          <Route path="cloud" element={<CloudPage />} />
        </Route>
      </Route>
    </Routes>
  );
}

/** Pages not yet redesigned draw inside a plain padded area. */
function PaddedPage() {
  return (
    <div className="p-6 lg:p-8">
      <Outlet />
    </div>
  );
}
