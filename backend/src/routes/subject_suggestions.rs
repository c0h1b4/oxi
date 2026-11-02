use std::sync::Arc;

use axum::{Extension, Json, extract::Query};
use serde::{Deserialize, Serialize};

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db::messages;
use crate::error::AppError;

#[derive(Deserialize)]
pub struct SubjectSuggestionsParams {
    #[serde(default)]
    pub q: String,
}

#[derive(Serialize)]
pub struct SubjectSuggestionsResponse {
    pub suggestions: Vec<String>,
}

pub async fn suggest_subjects(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Query(params): Query<SubjectSuggestionsParams>,
) -> Result<Json<SubjectSuggestionsResponse>, AppError> {
    if params.q.trim().chars().count() < 2 {
        return Ok(Json(SubjectSuggestionsResponse {
            suggestions: vec![],
        }));
    }
    let database = config
        .user_db(&session.user_hash)
        .await
        .map_err(AppError::InternalError)?;
    let suggestions = messages::suggest_subjects(&database, &params.q)
        .await
        .map_err(AppError::InternalError)?;
    Ok(Json(SubjectSuggestionsResponse { suggestions }))
}
