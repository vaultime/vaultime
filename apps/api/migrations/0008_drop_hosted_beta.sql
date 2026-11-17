-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: AGPL-3.0-or-later

-- The hosted beta is gone. A server takes new accounts when its operator
-- opens sign-ups, so invites, roles and beta applications go. access_state
-- and access_revoked_at stay, so an operator can still lock an account with
-- SQL.
ALTER TABLE cloud_accounts
    DROP COLUMN IF EXISTS invited_by_invite_id,
    DROP COLUMN IF EXISTS access_granted_at,
    DROP CONSTRAINT IF EXISTS cloud_accounts_role_check,
    DROP COLUMN IF EXISTS role;
DROP TABLE IF EXISTS cloud_invite_redemptions;
DROP TABLE IF EXISTS cloud_invites;
DROP TABLE IF EXISTS beta_applications;
