-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Two settings the first schema seeded that nothing ever read.

DELETE FROM settings WHERE key IN ('theme', 'preferred_image_mode');
