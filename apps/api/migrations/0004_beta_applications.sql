-- Applications for cloud beta access from the website. They are deleted once
-- answered, and after a retention period at the latest.
CREATE TABLE IF NOT EXISTS beta_applications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email CITEXT NOT NULL UNIQUE,
    platform TEXT NOT NULL,
    note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS beta_applications_created_at_idx
    ON beta_applications(created_at);
