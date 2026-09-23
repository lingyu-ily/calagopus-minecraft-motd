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
