-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: AGPL-3.0-or-later

-- A PC's name, sealed by the PC with the account's backup key, so the server
-- keeps it without being able to read it. device_name stays a plain label
-- for apps that seal no name.
ALTER TABLE cloud_devices ADD COLUMN sealed_name TEXT;
