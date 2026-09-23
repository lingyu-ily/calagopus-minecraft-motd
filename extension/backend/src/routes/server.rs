use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        autostart_on_join: bool,
    }

    #[utoipa::path(get, path = "/", responses((status = OK, body = inline(Response))), params(("server" = uuid::Uuid, description = "Server ID")))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
    ) -> ApiResponseResult {
        permissions.has_server_permission("settings.motd")?;
        let enabled: bool = sqlx::query_scalar(
            r#"
            SELECT COALESCE(
                (SELECT autostart_on_join
                 FROM ily_gfs_minecraftmotd_server_settings
                 WHERE server_uuid = $1),
                false
            )
            "#,
        )
        .bind(server.uuid)
        .fetch_one(state.database.read())
        .await?;

        ApiResponse::new_serialized(Response {
            autostart_on_join: enabled,
        })
        .ok()
    }
}

mod put {
    use serde::{Deserialize, Serialize};
    use shared::{
        GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub(super) struct Payload {
        autostart_on_join: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        autostart_on_join: bool,
    }

    #[utoipa::path(put, path = "/", responses((status = OK, body = inline(Response))), request_body = inline(Payload), params(("server" = uuid::Uuid, description = "Server ID")))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("settings.motd")?;

        let previous: bool = sqlx::query_scalar(
            r#"
            SELECT COALESCE(
                (SELECT autostart_on_join
                 FROM ily_gfs_minecraftmotd_server_settings
                 WHERE server_uuid = $1),
                false
            )
            "#,
        )
        .bind(server.uuid)
        .fetch_one(state.database.read())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO ily_gfs_minecraftmotd_server_settings
                (server_uuid, autostart_on_join, updated_at)
            VALUES ($1, $2, now())
            ON CONFLICT (server_uuid) DO UPDATE SET
                autostart_on_join = EXCLUDED.autostart_on_join,
                updated_at = now()
            "#,
        )
        .bind(server.uuid)
        .bind(data.autostart_on_join)
        .execute(state.database.write())
        .await?;

        activity_logger
            .log(
                "server:motd.update",
                serde_json::json!({
                    "autostart_on_join": {
                        "from": previous,
                        "to": data.autostart_on_join,
                    }
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {
            autostart_on_join: data.autostart_on_join,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(put::route))
        .with_state(state.clone())
}
