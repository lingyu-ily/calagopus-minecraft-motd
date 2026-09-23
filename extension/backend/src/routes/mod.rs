use shared::State;
use utoipa_axum::router::OpenApiRouter;

pub mod admin;
pub mod agent;
pub mod server;

pub fn admin_router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/settings", admin::settings_router(state))
        .nest("/nodes", admin::nodes_router(state))
        .with_state(state.clone())
}
