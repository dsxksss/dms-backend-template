//! v1 API 路由。

mod auth;
#[cfg(feature = "orgs")]
mod orgs;
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
        )
        .route(
            "/projects/{id}/members",
            get(projects::list_members).post(projects::add_member),
        )
        .route(
            "/projects/{id}/members/{user_id}",
            axum::routing::delete(projects::remove_member),
        );

    #[cfg(feature = "orgs")]
    let router = router
        .route("/orgs", post(orgs::create_org).get(orgs::list_orgs))
        .route("/orgs/{id}/members", post(orgs::add_org_member))
        .route("/orgs/{id}/teams", get(orgs::list_teams))
        .route("/teams", post(orgs::create_team))
        .route("/teams/{id}/members", post(orgs::add_team_member))
        .route(
            "/role-grants",
            post(orgs::grant_role).delete(orgs::revoke_role),
        )
        .route("/me/permissions", get(orgs::my_permissions));

    router
}
