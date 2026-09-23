use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod settings_get {
    use crate::settings::ExtensionSettingsData;
    use chrono::{DateTime, Utc};
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
    };
    use sqlx::Row;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    pub struct AgentNode {
        pub node_uuid: uuid::Uuid,
        pub node_name: String,
        pub enrolled: bool,
        pub version: Option<String>,
        pub last_seen: Option<DateTime<Utc>>,
        pub revoked_at: Option<DateTime<Utc>>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response<'a> {
        settings: &'a ExtensionSettingsData,
        nodes: Vec<AgentNode>,
    }

    #[utoipa::path(get, path = "/", responses((status = OK, body = inline(Response))))]
    pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
        permissions.has_admin_permission("extensions.manage")?;

        let settings = state.settings.get().await?;
        let extension_settings: &ExtensionSettingsData = settings.find_extension_settings()?;
        let rows = sqlx::query(
            r#"
            SELECT
                nodes.uuid AS node_uuid,
                nodes.name AS node_name,
                agents.node_uuid IS NOT NULL AS enrolled,
                agents.version,
                agents.last_seen,
                agents.revoked_at
            FROM nodes
            LEFT JOIN ily_gfs_minecraftmotd_agents AS agents
                ON agents.node_uuid = nodes.uuid
            ORDER BY nodes.name
            "#,
        )
        .fetch_all(state.database.read())
        .await?;

        let nodes = rows
            .into_iter()
            .map(|row| AgentNode {
                node_uuid: row.get("node_uuid"),
                node_name: row.get("node_name"),
                enrolled: row.get("enrolled"),
                version: row.get("version"),
                last_seen: row.get("last_seen"),
                revoked_at: row.get("revoked_at"),
            })
            .collect();

        ApiResponse::new_serialized(Response {
            settings: extension_settings,
            nodes,
        })
        .ok()
    }
}

mod settings_put {
    use crate::settings::ExtensionSettingsData;
    use serde::Serialize;
    use shared::{
        GetState,
        models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(put, path = "/", responses((status = OK, body = inline(Response))), request_body = inline(ExtensionSettingsData))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<ExtensionSettingsData>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("extensions.manage")?;
        data.validate()
            .map_err(|err| ApiResponse::error(err.to_string()))?;

        let mut settings = state.settings.get_mut().await?;
        let extension_settings: &mut ExtensionSettingsData =
            settings.find_mut_extension_settings()?;
        *extension_settings = data;
        settings.save().await?;

        activity_logger
            .log(
                "settings:extensions.minecraft-motd.update",
                serde_json::json!({ "extension": "ily.gfs.minecraftmotd" }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

mod enrollment_post {
    use crate::auth::token_hash;
    use axum::extract::Path;
    use chrono::{Duration, Utc};
    use serde::Serialize;
    use shared::{
        GetState,
        models::{
            ByUuid, admin_activity::GetAdminActivityLogger, node::Node, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        node_uuid: uuid::Uuid,
        enrollment_token: String,
        expires_at: chrono::DateTime<Utc>,
    }

    #[utoipa::path(post, path = "/{node}/enrollment-token", responses((status = OK, body = inline(Response))), params(("node" = uuid::Uuid, Path)))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        Path(node_uuid): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("extensions.manage")?;
        Node::by_uuid(&state.database, node_uuid).await?;

        let enrollment_token = hex::encode(rand::random::<[u8; 32]>());
        let token_hash = token_hash(&enrollment_token);
        let expires_at = Utc::now() + Duration::minutes(15);

        sqlx::query("DELETE FROM ily_gfs_minecraftmotd_enrollment_tokens WHERE node_uuid = $1")
            .bind(node_uuid)
            .execute(state.database.write())
            .await?;
        sqlx::query(
            r#"
            INSERT INTO ily_gfs_minecraftmotd_enrollment_tokens
                (node_uuid, token_hash, expires_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(node_uuid)
        .bind(token_hash)
        .bind(expires_at)
        .execute(state.database.write())
        .await?;

        activity_logger
            .log(
                "settings:extensions.minecraft-motd.enrollment-token",
                serde_json::json!({ "node_uuid": node_uuid }),
            )
            .await;

        ApiResponse::new_serialized(Response {
            node_uuid,
            enrollment_token,
            expires_at,
        })
        .ok()
    }
}

mod agent_delete {
    use axum::extract::Path;
    use serde::Serialize;
    use shared::{
        GetState,
        models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(delete, path = "/{node}/agent", responses((status = OK, body = inline(Response))), params(("node" = uuid::Uuid, Path)))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        Path(node_uuid): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("extensions.manage")?;
        sqlx::query(
            "UPDATE ily_gfs_minecraftmotd_agents SET revoked_at = now() WHERE node_uuid = $1",
        )
        .bind(node_uuid)
        .execute(state.database.write())
        .await?;

        activity_logger
            .log(
                "settings:extensions.minecraft-motd.agent-revoke",
                serde_json::json!({ "node_uuid": node_uuid }),
            )
            .await;
        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn settings_router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(settings_get::route))
        .routes(routes!(settings_put::route))
        .with_state(state.clone())
}

pub fn nodes_router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(enrollment_post::route))
        .routes(routes!(agent_delete::route))
        .with_state(state.clone())
}
