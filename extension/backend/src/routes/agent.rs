use super::State;
use crate::{
    auth::{authenticate_agent, token_hash},
    settings::ExtensionSettingsData,
    state::{EffectiveState, is_excluded, resolve_state},
};
use axum::http::StatusCode;
use shared::models::server::ServerStatus;
use sqlx::Row;
use utoipa_axum::{router::OpenApiRouter, routes};

fn parse_server_status(value: Option<&str>) -> Option<ServerStatus> {
    match value?.to_ascii_lowercase().as_str() {
        "installing" => Some(ServerStatus::Installing),
        "install_failed" => Some(ServerStatus::InstallFailed),
        "restoring_backup" => Some(ServerStatus::RestoringBackup),
        "backup_restore_failed" => Some(ServerStatus::BackupRestoreFailed),
        _ => None,
    }
}

mod enroll {
    use super::*;
    use chrono::Utc;
    use serde::{Deserialize, Serialize};
    use shared::{
        GetState,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub(super) struct Payload {
        enrollment_token: String,
        version: String,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        node_uuid: uuid::Uuid,
        agent_token: String,
        panel_time: chrono::DateTime<Utc>,
    }

    #[utoipa::path(post, path = "/enroll", responses((status = OK, body = inline(Response))), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if data.version.len() > 64 || data.enrollment_token.len() > 256 {
            return ApiResponse::error("invalid enrollment request")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        let mut transaction = state.database.write().begin().await?;
        let enrollment_hash = token_hash(&data.enrollment_token);
        let row = sqlx::query(
            r#"
            DELETE FROM ily_gfs_minecraftmotd_enrollment_tokens
            WHERE token_hash = $1 AND expires_at > now()
            RETURNING node_uuid
            "#,
        )
        .bind(enrollment_hash)
        .fetch_optional(&mut *transaction)
        .await?;

        let Some(row) = row else {
            transaction.rollback().await?;
            return ApiResponse::error("invalid or expired enrollment token")
                .with_status(StatusCode::UNAUTHORIZED)
                .ok();
        };
        let node_uuid: uuid::Uuid = row.get("node_uuid");
        let agent_token = hex::encode(rand::random::<[u8; 32]>());
        let credential_hash = token_hash(&agent_token);

        sqlx::query(
            r#"
            INSERT INTO ily_gfs_minecraftmotd_agents
                (node_uuid, credential_hash, version, enrolled_at, last_seen, revoked_at)
            VALUES ($1, $2, $3, now(), now(), NULL)
            ON CONFLICT (node_uuid) DO UPDATE SET
                credential_hash = EXCLUDED.credential_hash,
                version = EXCLUDED.version,
                enrolled_at = now(),
                last_seen = now(),
                revoked_at = NULL
            "#,
        )
        .bind(node_uuid)
        .bind(credential_hash)
        .bind(data.version)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        tracing::info!(%node_uuid, "Minecraft MOTD node agent enrolled");
        ApiResponse::new_serialized(Response {
            node_uuid,
            agent_token,
            panel_time: Utc::now(),
        })
        .ok()
    }
}

mod snapshot {
    use super::*;
    use axum::http::HeaderMap;
    use chrono::Utc;
    use serde::Serialize;
    use shared::{
        GetState,
        models::{ByUuid, node::Node},
        response::{ApiResponse, ApiResponseResult},
    };
    use std::collections::HashMap;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct SnapshotServer {
        server_uuid: uuid::Uuid,
        server_name: String,
        node_name: String,
        allocation_ip: String,
        allocation_port: u16,
        state: EffectiveState,
        autostart_on_join: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        revision: i64,
        generated_at: chrono::DateTime<Utc>,
        node_uuid: uuid::Uuid,
        node_reachable: bool,
        settings: ExtensionSettingsData,
        servers: Vec<SnapshotServer>,
    }

    #[utoipa::path(get, path = "/snapshot", responses((status = OK, body = inline(Response)), (status = UNAUTHORIZED)))]
    pub async fn route(state: GetState, headers: HeaderMap) -> ApiResponseResult {
        let node_uuid = authenticate_agent(&state, &headers).await?;
        let settings_guard = state.settings.get().await?;
        let settings: ExtensionSettingsData = settings_guard
            .find_extension_settings::<ExtensionSettingsData>()?
            .clone();
        drop(settings_guard);

        let node = Node::by_uuid(&state.database, node_uuid).await?;
        let resources = match node.api_client(&state.database).await {
            Ok(client) => client.get_servers_utilization().await.ok(),
            Err(_) => None,
        };
        let node_reachable = resources.is_some();
        let resources: HashMap<uuid::Uuid, wings_api::ResourceUsage> =
            resources.unwrap_or_default().into_iter().collect();

        let rows = sqlx::query(
            r#"
            SELECT
                servers.uuid AS server_uuid,
                servers.name AS server_name,
                servers.egg_uuid,
                servers.status::text AS status,
                servers.suspended,
                servers.destination_node_uuid IS NOT NULL AS transferring,
                nodes.name AS node_name,
                nodes.maintenance_enabled,
                host(node_allocations.ip) AS allocation_ip,
                node_allocations.port AS allocation_port,
                servers.allocation_uuid = server_allocations.uuid AS is_primary,
                COALESCE(motd.autostart_on_join, false) AS autostart_on_join
            FROM servers
            JOIN nodes ON nodes.uuid = servers.node_uuid
            JOIN server_allocations ON server_allocations.server_uuid = servers.uuid
            JOIN node_allocations ON node_allocations.uuid = server_allocations.allocation_uuid
            LEFT JOIN ily_gfs_minecraftmotd_server_settings AS motd
                ON motd.server_uuid = servers.uuid
            WHERE servers.node_uuid = $1
            ORDER BY servers.uuid, node_allocations.port
            "#,
        )
        .bind(node_uuid)
        .fetch_all(state.database.read())
        .await?;

        let mut servers = Vec::new();
        for row in rows {
            let server_uuid: uuid::Uuid = row.get("server_uuid");
            let egg_uuid: uuid::Uuid = row.get("egg_uuid");
            let is_primary: bool = row.get("is_primary");
            if (!settings.all_allocations && !is_primary)
                || is_excluded(&settings, server_uuid, egg_uuid)
            {
                continue;
            }

            let wings_state = resources.get(&server_uuid).map(|usage| usage.state);
            let raw_status: Option<String> = row.get("status");
            let effective_state = resolve_state(
                row.get("suspended"),
                row.get("maintenance_enabled"),
                row.get("transferring"),
                parse_server_status(raw_status.as_deref()),
                node_reachable,
                wings_state,
            );

            let port: i32 = row.get("allocation_port");
            if !(1..=u16::MAX as i32).contains(&port) {
                continue;
            }
            servers.push(SnapshotServer {
                server_uuid,
                server_name: row.get("server_name"),
                node_name: row.get("node_name"),
                allocation_ip: row.get("allocation_ip"),
                allocation_port: port as u16,
                state: effective_state,
                autostart_on_join: row.get("autostart_on_join"),
            });
        }

        sqlx::query(
            "UPDATE ily_gfs_minecraftmotd_agents SET last_seen = now() WHERE node_uuid = $1",
        )
        .bind(node_uuid)
        .execute(state.database.write())
        .await?;

        let generated_at = Utc::now();
        ApiResponse::new_serialized(Response {
            revision: generated_at.timestamp_millis(),
            generated_at,
            node_uuid,
            node_reachable,
            settings,
            servers,
        })
        .ok()
    }
}

mod heartbeat {
    use super::*;
    use axum::http::HeaderMap;
    use serde::{Deserialize, Serialize};
    use shared::{
        GetState,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub(super) struct Payload {
        version: String,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(post, path = "/heartbeat", responses((status = OK, body = inline(Response)), (status = UNAUTHORIZED)), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        headers: HeaderMap,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        let node_uuid = authenticate_agent(&state, &headers).await?;
        if data.version.len() > 64 {
            return ApiResponse::error("invalid agent version")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        sqlx::query(
            "UPDATE ily_gfs_minecraftmotd_agents SET version = $2, last_seen = now() WHERE node_uuid = $1",
        )
        .bind(node_uuid)
        .bind(data.version)
        .execute(state.database.write())
        .await?;
        ApiResponse::new_serialized(Response {}).ok()
    }
}

mod wake {
    use super::*;
    use axum::http::HeaderMap;
    use serde::{Deserialize, Serialize};
    use shared::{
        GetState,
        models::{ByUuid, node::Node, server::Server},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub(super) struct Payload {
        server_uuid: uuid::Uuid,
        allocation_port: u16,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        started: bool,
        reason: &'static str,
    }

    fn response(started: bool, reason: &'static str) -> ApiResponseResult {
        ApiResponse::new_serialized(Response { started, reason }).ok()
    }

    #[utoipa::path(post, path = "/wake", responses((status = OK, body = inline(Response)), (status = UNAUTHORIZED)), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        headers: HeaderMap,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        let node_uuid = authenticate_agent(&state, &headers).await?;
        let settings_guard = state.settings.get().await?;
        let settings: ExtensionSettingsData = settings_guard
            .find_extension_settings::<ExtensionSettingsData>()?
            .clone();
        drop(settings_guard);
        if !settings.enabled {
            return response(false, "extension_disabled");
        }

        let row = sqlx::query(
            r#"
            SELECT servers.egg_uuid
            FROM servers
            JOIN server_allocations ON server_allocations.server_uuid = servers.uuid
            JOIN node_allocations ON node_allocations.uuid = server_allocations.allocation_uuid
            WHERE servers.uuid = $1
              AND servers.node_uuid = $2
              AND node_allocations.port = $3
              AND ($4 OR servers.allocation_uuid = server_allocations.uuid)
            "#,
        )
        .bind(data.server_uuid)
        .bind(node_uuid)
        .bind(data.allocation_port as i32)
        .bind(settings.all_allocations)
        .fetch_optional(state.database.read())
        .await?;
        let Some(row) = row else {
            return response(false, "allocation_not_authorized");
        };
        let egg_uuid: uuid::Uuid = row.get("egg_uuid");
        if is_excluded(&settings, data.server_uuid, egg_uuid) {
            return response(false, "excluded");
        }

        let server = Server::by_uuid(&state.database, data.server_uuid).await?;
        if server.node.uuid != node_uuid {
            return response(false, "wrong_node");
        }
        let node = Node::by_uuid(&state.database, node_uuid).await?;
        if node.maintenance_enabled || server.unavailable_reason().is_some() {
            return response(false, "server_unavailable");
        }

        let client = node.api_client(&state.database).await?;
        let utilization = client.get_servers_utilization().await?;
        if utilization
            .get(&server.uuid)
            .is_some_and(|usage| !matches!(usage.state, wings_api::ServerState::Offline))
        {
            return response(false, "not_offline");
        }

        let locked = sqlx::query_scalar::<_, uuid::Uuid>(
            r#"
            UPDATE ily_gfs_minecraftmotd_server_settings
            SET last_wake_at = now()
            WHERE server_uuid = $1
              AND autostart_on_join
              AND (last_wake_at IS NULL OR last_wake_at < now() - interval '30 seconds')
            RETURNING server_uuid
            "#,
        )
        .bind(server.uuid)
        .fetch_optional(state.database.write())
        .await?;
        if locked.is_none() {
            return response(false, "disabled_or_throttled");
        }

        if let Err(err) = client
            .post_servers_server_power(
                server.uuid,
                &wings_api::servers_server_power::post::RequestBody {
                    action: wings_api::ServerPowerAction::Start,
                    wait_seconds: None,
                },
            )
            .await
        {
            sqlx::query(
                "UPDATE ily_gfs_minecraftmotd_server_settings SET last_wake_at = NULL WHERE server_uuid = $1",
            )
            .bind(server.uuid)
            .execute(state.database.write())
            .await?;
            return Err(ApiResponse::error(format!(
                "failed to start server: {err:?}"
            )));
        }

        tracing::info!(server = %server.uuid, node = %node_uuid, "server started by Minecraft login handshake");
        response(true, "started")
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(enroll::route))
        .routes(routes!(snapshot::route))
        .routes(routes!(heartbeat::route))
        .routes(routes!(wake::route))
        .with_state(state.clone())
}
