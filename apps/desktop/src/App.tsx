// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Routes, Route, Navigate } from "react-router";
import { AppLayout } from "@/components/layout/AppLayout";
import { LibraryPage } from "@/features/library/LibraryPage";
import { JournalPage } from "@/features/journal/JournalPage";
import { CloudPage } from "@/features/cloud/CloudPage";
import { SettingsPage } from "@/features/settings/SettingsPage";
import { GameDetailsPage } from "@/features/game-details/GameDetailsPage";

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
        <Route path="cloud" element={<CloudPage />} />
      </Route>
    </Routes>
  );
}

