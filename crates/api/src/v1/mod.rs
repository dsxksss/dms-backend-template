//! v1 API 路由。

mod auth;
#[cfg(feature = "project")]
mod projects;

use axum::Router;
use axum::routing::{get, post};

use crate::state::AppState;

/// 组装 `/v1` 子路由。
pub fn router() -> Router<AppState> {
    let router = Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/token/exchange", post(auth::exchange))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/logout", post(auth::logout))
        .route("/me", get(auth::me));

    #[cfg(feature = "project")]
    let router = router
        .route("/projects", post(projects::create).get(projects::list))
        .route(
            "/projects/{id}",
            get(projects::get)
                .patch(projects::update)
                .delete(projects::delete),
        );

    router
}
