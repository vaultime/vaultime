// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Routes, Route, Navigate } from "react-router";
import { AppLayout } from "@/components/layout/AppLayout";
import { HomePage } from "@/features/library/HomePage";
import { LibraryPage } from "@/features/library/LibraryPage";
import { JournalPage } from "@/features/journal/JournalPage";
import { StatsPage } from "@/features/stats/StatsPage";
import { YearInGamesPage } from "@/features/stats/YearInGamesPage";
import { CloudPage } from "@/features/cloud/CloudPage";
import { SettingsPage } from "@/features/settings/SettingsPage";
import { GameDetailsPage } from "@/features/game-details/GameDetailsPage";
import { useCloudSession } from "@/features/cloud/cloud-context";

export function App() {
  const { enabled: cloudEnabled } = useCloudSession();
  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route index element={<Navigate to="/home" replace />} />
        <Route path="home" element={<HomePage />} />
        <Route path="library" element={<LibraryPage />} />
        <Route path="library/:gameId" element={<GameDetailsPage />} />
        <Route path="journal" element={<JournalPage />} />
        <Route path="stats" element={<StatsPage />} />
        <Route path="stats/year/:year?" element={<YearInGamesPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route
          path="cloud"
          element={cloudEnabled ? <CloudPage /> : <Navigate to="/settings#cloud-backup" replace />}
        />
      </Route>
    </Routes>
  );
}

