CREATE TABLE IF NOT EXISTS ily_gfs_minecraftmotd_server_settings (
    server_uuid UUID PRIMARY KEY
        REFERENCES servers(uuid) ON DELETE CASCADE,
    autostart_on_join BOOLEAN NOT NULL DEFAULT FALSE,
    last_wake_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS ily_gfs_minecraftmotd_agents (
    node_uuid UUID PRIMARY KEY
        REFERENCES nodes(uuid) ON DELETE CASCADE,
    credential_hash BYTEA NOT NULL UNIQUE,
    version VARCHAR(64) NOT NULL,
    enrolled_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS ily_gfs_minecraftmotd_enrollment_tokens (
    node_uuid UUID PRIMARY KEY
        REFERENCES nodes(uuid) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS ily_gfs_minecraftmotd_agents_last_seen_idx
    ON ily_gfs_minecraftmotd_agents(last_seen);

CREATE INDEX IF NOT EXISTS ily_gfs_minecraftmotd_enrollment_expires_idx
    ON ily_gfs_minecraftmotd_enrollment_tokens(expires_at);

