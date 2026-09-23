use base64::Engine;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use shared::extensions::settings::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use utoipa::ToSchema;

pub const STATE_NAMES: [&str; 11] = [
    "offline",
    "starting",
    "stopping",
    "suspended",
    "node_maintenance",
    "transferring",
    "installing",
    "install_failed",
    "restoring_backup",
    "backup_restore_failed",
    "node_unreachable",
];

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MotdStateSettings {
    pub version: compact_str::CompactString,
    pub protocol: Option<i32>,
    pub online_players: i32,
    pub max_players: i32,
    pub descriptions: Vec<compact_str::CompactString>,
    pub kick_message: compact_str::CompactString,
}

impl MotdStateSettings {
    fn new(version: &str, color: &str, message: &str) -> Self {
        Self {
            version: version.into(),
            protocol: None,
            online_players: 0,
            max_players: 0,
            descriptions: vec![format!("{color}§l{version}\n§7{message}").into()],
            kick_message: format!("{color}§l{version}\n§7{message}").into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExtensionSettingsData {
    pub enabled: bool,
    pub all_allocations: bool,
    pub rotation_interval_seconds: u64,
    pub swap_lines: bool,
    pub gradient_enabled: bool,
    pub gradient_colors: Vec<compact_str::CompactString>,
    pub favicon_base64: Option<compact_str::CompactString>,
    pub excluded_egg_uuids: Vec<uuid::Uuid>,
    pub excluded_server_uuids: Vec<uuid::Uuid>,
    pub states: IndexMap<compact_str::CompactString, MotdStateSettings>,
}

impl Default for ExtensionSettingsData {
    fn default() -> Self {
        let mut states = IndexMap::new();
        states.insert(
            "offline".into(),
            MotdStateSettings::new("Offline", "§c", "This server is currently offline."),
        );
        states.insert(
            "starting".into(),
            MotdStateSettings::new(
                "Starting",
                "§a",
                "The server is waking up. Please reconnect shortly.",
            ),
        );
        states.insert(
            "stopping".into(),
            MotdStateSettings::new("Stopping", "§6", "The server is shutting down."),
        );
        states.insert(
            "suspended".into(),
            MotdStateSettings::new(
                "Suspended",
                "§c",
                "Please contact the server administrator.",
            ),
        );
        states.insert(
            "node_maintenance".into(),
            MotdStateSettings::new(
                "Maintenance",
                "§e",
                "The node is currently under maintenance.",
            ),
        );
        states.insert(
            "transferring".into(),
            MotdStateSettings::new("Transferring", "§b", "The server is being transferred."),
        );
        states.insert(
            "installing".into(),
            MotdStateSettings::new(
                "Installing",
                "§b",
                "The server is currently being installed.",
            ),
        );
        states.insert(
            "install_failed".into(),
            MotdStateSettings::new("Install Failed", "§c", "The server installation failed."),
        );
        states.insert(
            "restoring_backup".into(),
            MotdStateSettings::new(
                "Restoring Backup",
                "§d",
                "A backup is currently being restored.",
            ),
        );
        states.insert(
            "backup_restore_failed".into(),
            MotdStateSettings::new("Restore Failed", "§c", "The backup restore failed."),
        );
        states.insert(
            "node_unreachable".into(),
            MotdStateSettings::new(
                "Node Unreachable",
                "§4",
                "The node is temporarily unreachable.",
            ),
        );

        Self {
            enabled: true,
            all_allocations: false,
            rotation_interval_seconds: 10,
            swap_lines: false,
            gradient_enabled: false,
            gradient_colors: vec!["#22d3ee".into(), "#f59e0b".into()],
            favicon_base64: None,
            excluded_egg_uuids: Vec::new(),
            excluded_server_uuids: Vec::new(),
            states,
        }
    }
}

impl ExtensionSettingsData {
    pub fn validate(&self) -> Result<(), anyhow::Error> {
        if !(1..=3600).contains(&self.rotation_interval_seconds) {
            anyhow::bail!("rotation_interval_seconds must be between 1 and 3600");
        }
        for state in STATE_NAMES {
            let Some(config) = self.states.get(state) else {
                anyhow::bail!("missing MOTD state: {state}");
            };
            if config.descriptions.is_empty() {
                anyhow::bail!("MOTD state {state} must have at least one description");
            }
            if config.max_players < 0 || config.online_players < 0 {
                anyhow::bail!("player counts cannot be negative");
            }
        }
        for color in &self.gradient_colors {
            let value = color.as_str();
            if value.len() != 7
                || !value.starts_with('#')
                || !value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                anyhow::bail!("gradient colors must use #RRGGBB format");
            }
        }
        if let Some(icon) = &self.favicon_base64 {
            if icon.len() > 350_000 {
                anyhow::bail!("favicon is too large");
            }
            let png = base64::engine::general_purpose::STANDARD
                .decode(icon.as_bytes())
                .map_err(|_| anyhow::anyhow!("favicon is not valid base64"))?;
            if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" || &png[12..16] != b"IHDR" {
                anyhow::bail!("favicon must be a PNG image");
            }
            let width = u32::from_be_bytes(png[16..20].try_into()?);
            let height = u32::from_be_bytes(png[20..24].try_into()?);
            if width != 64 || height != 64 {
                anyhow::bail!("favicon must be exactly 64x64 pixels");
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for ExtensionSettingsData {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer.write_serde_setting("configuration", self)?)
    }
}

pub struct ExtensionSettingsDataDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for ExtensionSettingsDataDeserializer {
    async fn deserialize_boxed(
        &self,
        deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let settings: ExtensionSettingsData = deserializer
            .read_serde_setting("configuration")
            .unwrap_or_default();
        Ok(Box::new(settings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_every_intercepted_state() {
        let settings = ExtensionSettingsData::default();
        settings.validate().unwrap();
        assert!(!settings.all_allocations);
        assert_eq!(settings.states.len(), STATE_NAMES.len());
        assert!(
            STATE_NAMES
                .iter()
                .all(|state| settings.states.contains_key(*state))
        );
    }

    #[test]
    fn rejects_non_64_pixel_favicon() {
        let mut settings = ExtensionSettingsData::default();
        let mut png_header = Vec::from(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".as_slice());
        png_header.extend_from_slice(&32_u32.to_be_bytes());
        png_header.extend_from_slice(&64_u32.to_be_bytes());
        settings.favicon_base64 = Some(
            base64::engine::general_purpose::STANDARD
                .encode(png_header)
                .into(),
        );
        assert!(settings.validate().is_err());
    }
}
