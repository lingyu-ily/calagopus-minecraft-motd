use shared::{
    State,
    extensions::{
        Extension, ExtensionPermissionsBuilder, ExtensionRouteBuilder,
        settings::ExtensionSettingsDeserializer,
    },
};
use std::sync::Arc;

mod auth;
mod routes;
mod settings;
mod state;

#[derive(Default)]
pub struct ExtensionStruct;

#[async_trait::async_trait]
impl Extension for ExtensionStruct {
    async fn initialize(&mut self, _state: State) {
        tracing::info!("ily.gfs.minecraftmotd initialized");
    }

    async fn settings_deserializer(&self, _state: State) -> ExtensionSettingsDeserializer {
        Arc::new(settings::ExtensionSettingsDataDeserializer)
    }

    async fn initialize_permissions(
        &mut self,
        _state: State,
        builder: ExtensionPermissionsBuilder,
    ) -> ExtensionPermissionsBuilder {
        builder.mutate_server_permission_group("settings", |group| {
            group.permissions.insert(
                "motd",
                "Allows viewing and changing Minecraft MOTD wake-on-login settings.",
            );
        })
    }

    async fn initialize_router(
        &mut self,
        state: State,
        builder: ExtensionRouteBuilder,
    ) -> ExtensionRouteBuilder {
        builder
            .add_admin_api_router(|router| {
                router.nest("/minecraft-motd", routes::admin_router(&state))
            })
            .add_client_server_api_router(|router| {
                router.nest("/minecraft-motd", routes::server::router(&state))
            })
            .add_global_router(|router| {
                router.nest(
                    "/api/minecraft-motd/agent/v1",
                    routes::agent::router(&state),
                )
            })
    }
}
