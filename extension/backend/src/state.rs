use crate::settings::ExtensionSettingsData;
use serde::Serialize;
use shared::models::server::ServerStatus;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveState {
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

pub fn resolve_state(
    suspended: bool,
    node_maintenance: bool,
    transferring: bool,
    status: Option<ServerStatus>,
    node_reachable: bool,
    wings_state: Option<wings_api::ServerState>,
) -> EffectiveState {
    if suspended {
        return EffectiveState::Suspended;
    }
    if node_maintenance {
        return EffectiveState::NodeMaintenance;
    }
    if transferring {
        return EffectiveState::Transferring;
    }
    match status {
        Some(ServerStatus::Installing) => return EffectiveState::Installing,
        Some(ServerStatus::InstallFailed) => return EffectiveState::InstallFailed,
        Some(ServerStatus::RestoringBackup) => return EffectiveState::RestoringBackup,
        Some(ServerStatus::BackupRestoreFailed) => return EffectiveState::BackupRestoreFailed,
        None => {}
    }
    if !node_reachable {
        return EffectiveState::NodeUnreachable;
    }
    match wings_state {
        Some(wings_api::ServerState::Running) => EffectiveState::Running,
        Some(wings_api::ServerState::Starting) => EffectiveState::Starting,
        Some(wings_api::ServerState::Stopping) => EffectiveState::Stopping,
        Some(wings_api::ServerState::Offline) | None => EffectiveState::Offline,
    }
}

pub fn is_excluded(
    settings: &ExtensionSettingsData,
    server_uuid: uuid::Uuid,
    egg_uuid: uuid::Uuid,
) -> bool {
    !settings.enabled
        || settings.excluded_server_uuids.contains(&server_uuid)
        || settings.excluded_egg_uuids.contains(&egg_uuid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn administrative_states_win_over_power_state() {
        assert_eq!(
            resolve_state(
                true,
                false,
                false,
                None,
                true,
                Some(wings_api::ServerState::Running)
            ),
            EffectiveState::Suspended
        );
        assert_eq!(
            resolve_state(
                false,
                true,
                false,
                None,
                true,
                Some(wings_api::ServerState::Running)
            ),
            EffectiveState::NodeMaintenance
        );
    }

    #[test]
    fn fixed_priority_is_applied_before_wings_power_state() {
        assert_eq!(
            resolve_state(
                false,
                false,
                true,
                Some(ServerStatus::InstallFailed),
                false,
                Some(wings_api::ServerState::Running),
            ),
            EffectiveState::Transferring
        );
        assert_eq!(
            resolve_state(
                false,
                false,
                false,
                Some(ServerStatus::RestoringBackup),
                false,
                Some(wings_api::ServerState::Running),
            ),
            EffectiveState::RestoringBackup
        );
        assert_eq!(
            resolve_state(
                false,
                false,
                false,
                None,
                false,
                Some(wings_api::ServerState::Running),
            ),
            EffectiveState::NodeUnreachable
        );
        assert_eq!(
            resolve_state(
                false,
                false,
                false,
                None,
                true,
                Some(wings_api::ServerState::Stopping),
            ),
            EffectiveState::Stopping
        );
    }
}
