-- The server stores no IP addresses. No code ever wrote these columns.
ALTER TABLE cloud_refresh_tokens DROP COLUMN IF EXISTS last_used_ip;
ALTER TABLE cloud_invite_redemptions DROP COLUMN IF EXISTS source_ip;
