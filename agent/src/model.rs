use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::IpAddr};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Snapshot {
    pub revision: i64,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub node_uuid: uuid::Uuid,
    pub node_reachable: bool,
    pub settings: GlobalSettings,
    pub servers: Vec<SnapshotServer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SnapshotServer {
    pub server_uuid: uuid::Uuid,
    pub server_name: String,
    pub node_name: String,
    pub allocation_ip: IpAddr,
    pub allocation_port: u16,
    pub state: ServerState,
    pub autostart_on_join: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerState {
    Offline,
    Starting,
    Stopping,
    Running,
    Suspended,
    NodeMaintenance,
    Transferring,
    Installing,
    InstallFailed,
    RestoringBackup,
    BackupRestoreFailed,
    NodeUnreachable,
}

impl ServerState {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Starting => "starting",
            Self::Stopping => "stopping",
            Self::Running => "running",
            Self::Suspended => "suspended",
            Self::NodeMaintenance => "node_maintenance",
            Self::Transferring => "transferring",
            Self::Installing => "installing",
            Self::InstallFailed => "install_failed",
            Self::RestoringBackup => "restoring_backup",
            Self::BackupRestoreFailed => "backup_restore_failed",
            Self::NodeUnreachable => "node_unreachable",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GlobalSettings {
    pub enabled: bool,
    #[serde(rename = "all_allocations")]
    pub _all_allocations: bool,
    pub rotation_interval_seconds: u64,
    pub swap_lines: bool,
    pub gradient_enabled: bool,
    pub gradient_colors: Vec<String>,
    pub favicon_base64: Option<String>,
    #[serde(rename = "excluded_egg_uuids")]
    pub _excluded_egg_uuids: Vec<uuid::Uuid>,
    #[serde(rename = "excluded_server_uuids")]
    pub _excluded_server_uuids: Vec<uuid::Uuid>,
    pub states: HashMap<String, MotdStateSettings>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MotdStateSettings {
    pub version: String,
    pub protocol: Option<i32>,
    pub online_players: i32,
    pub max_players: i32,
    pub descriptions: Vec<String>,
    pub kick_message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct WakeRequest {
    pub server_uuid: uuid::Uuid,
    pub allocation_port: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WakeResponse {
    pub started: bool,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct EnrollmentRequest<'a> {
    pub enrollment_token: &'a str,
    pub version: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EnrollmentResponse {
    pub node_uuid: uuid::Uuid,
    pub agent_token: String,
}

#[derive(Debug, Serialize)]
pub struct HeartbeatRequest<'a> {
    pub version: &'a str,
}
