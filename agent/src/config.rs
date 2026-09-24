use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub panel_url: String,
    pub agent_token: String,
    pub node_uuid: uuid::Uuid,
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
    #[serde(default = "default_stale_after")]
    pub stale_after_seconds: u64,
    #[serde(default)]
    pub firewall_backend: FirewallPreference,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirewallPreference {
    #[default]
    Auto,
    Nftables,
    Iptables,
}

const fn default_listen_port() -> u16 {
    4001
}

const fn default_poll_interval() -> u64 {
    2
}

const fn default_stale_after() -> u64 {
    15
}

impl AgentConfig {
    pub async fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: Self = toml::from_str(&raw).context("invalid agent configuration")?;
        config.validate()?;
        Ok(config)
    }

    pub async fn save(&self, path: &Path) -> anyhow::Result<()> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let raw = toml::to_string_pretty(self)?;
        tokio::fs::write(path, raw).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await?;
        }
        Ok(())
    }

    pub fn apply_env_overrides(&mut self) -> anyhow::Result<()> {
        if let Some(value) = env_override("MOTD_PANEL_URL")? {
            let value = value.trim().trim_end_matches('/');
            if value.is_empty() {
                anyhow::bail!("MOTD_PANEL_URL must not be empty");
            }
            self.panel_url = value.to_owned();
        }
        if let Some(value) = env_override("MOTD_LISTEN_PORT")? {
            self.listen_port = value
                .parse()
                .context("MOTD_LISTEN_PORT must be a valid TCP port")?;
        }
        if let Some(value) = env_override("MOTD_POLL_INTERVAL_SECONDS")? {
            self.poll_interval_seconds = value
                .parse()
                .context("MOTD_POLL_INTERVAL_SECONDS must be a positive integer")?;
        }
        if let Some(value) = env_override("MOTD_STALE_AFTER_SECONDS")? {
            self.stale_after_seconds = value
                .parse()
                .context("MOTD_STALE_AFTER_SECONDS must be a positive integer")?;
        }
        if let Some(value) = env_override("MOTD_FIREWALL_BACKEND")? {
            self.firewall_backend = match value.as_str() {
                "auto" => FirewallPreference::Auto,
                "nftables" => FirewallPreference::Nftables,
                "iptables" => FirewallPreference::Iptables,
                _ => anyhow::bail!("MOTD_FIREWALL_BACKEND must be auto, nftables, or iptables"),
            };
        }
        self.validate()
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.agent_token.len() < 32 {
            anyhow::bail!("agent token is invalid");
        }
        if self.listen_port == 0 {
            anyhow::bail!("listen_port must not be zero");
        }
        if !(1..=60).contains(&self.poll_interval_seconds) {
            anyhow::bail!("poll_interval_seconds must be between 1 and 60");
        }
        if self.stale_after_seconds < self.poll_interval_seconds {
            anyhow::bail!("stale_after_seconds must not be shorter than the poll interval");
        }
        Ok(())
    }
}

fn env_override(key: &str) -> anyhow::Result<Option<String>> {
    match std::env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error).with_context(|| format!("{key} is invalid")),
    }
}
