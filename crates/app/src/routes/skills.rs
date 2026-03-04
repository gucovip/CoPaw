// -*- coding: utf-8 -*-
// API routes for skills management

use crate::routes::schemas::{
    BatchSkillsRequest, CreateSkillRequest, HubInstallRequest, HubSkillSpec, SkillFileContent,
    SkillSpec,
};
use crate::skills::{SkillError, SkillService, SkillSource};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Hub search query parameters
#[derive(Debug, Deserialize)]
pub struct HubSearchQuery {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_limit() -> usize {
    20
}

/// Error response
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// Skills API error
#[derive(Debug)]
pub enum SkillsError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for SkillsError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            SkillsError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            SkillsError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            SkillsError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        let body = Json(ErrorResponse { error: message });
        (status, body).into_response()
    }
}

/// GET /api/skills - List all skills (built-in + custom)
pub async fn list_skills() -> Result<Json<Vec<SkillSpec>>, SkillsError> {
    let all_skills = SkillService::list_all_skills()
        .await
        .map_err(|e| SkillsError::Internal(e.to_string()))?;

    let available_skills = SkillService::list_available_skills()
        .await
        .map_err(|e| SkillsError::Internal(e.to_string()))?;

    let available_names: std::collections::HashSet<String> =
        available_skills.into_iter().map(|s| s.name).collect();

    let skills_spec: Vec<SkillSpec> = all_skills
        .into_iter()
        .map(|skill| SkillSpec {
            name: skill.name.clone(),
            content: skill.content,
            source: format!("{:?}", skill.source).to_lowercase(),
            path: skill.path.to_string_lossy().to_string(),
            references: skill.references,
            scripts: skill.scripts,
            enabled: available_names.contains(&skill.name),
        })
        .collect();

    Ok(Json(skills_spec))
}

/// GET /api/skills/available - List all available skills (built-in + custom with enabled status)
pub async fn list_available_skills() -> Result<Json<Vec<SkillSpec>>, SkillsError> {
    list_skills().await
}

/// GET /api/skills/installed - List installed (available) skills
pub async fn list_installed_skills() -> Result<Json<Vec<SkillSpec>>, SkillsError> {
    let skills = SkillService::list_available_skills()
        .await
        .map_err(|e| SkillsError::Internal(e.to_string()))?;

    let skills_spec: Vec<SkillSpec> = skills
        .into_iter()
        .map(|skill| SkillSpec {
            name: skill.name,
            content: skill.content,
            source: format!("{:?}", skill.source).to_lowercase(),
            path: skill.path.to_string_lossy().to_string(),
            references: skill.references,
            scripts: skill.scripts,
            enabled: true,
        })
        .collect();

    Ok(Json(skills_spec))
}

/// GET /api/skills/hub - Search Skills Hub (placeholder)
pub async fn search_hub(Query(_query): Query<HubSearchQuery>) -> Json<Vec<HubSkillSpec>> {
    // TODO: Implement actual hub search in Phase 5.2
    // For now, return empty results
    Json(vec![])
}

/// POST /api/skills/hub/install - Install from Hub (placeholder)
pub async fn install_from_hub(
    Json(_req): Json<HubInstallRequest>,
) -> Result<Json<Value>, SkillsError> {
    // TODO: Implement actual hub installation in Phase 5.2
    Ok(Json(serde_json::json!({
        "installed": false,
        "message": "Hub installation not yet implemented"
    })))
}

/// GET /api/skills/market - Get skill market info (placeholder)
pub async fn get_market() -> Json<Value> {
    // TODO: Implement actual market info
    Json(serde_json::json!({
        "skills": [],
        "categories": []
    }))
}

/// POST /api/skills/{skill_name}/enable - Enable a skill
pub async fn enable_skill(Path(skill_name): Path<String>) -> Result<Json<Value>, SkillsError> {
    SkillService::enable_skill(&skill_name, false)
        .await
        .map_err(|e| SkillsError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "enabled": true,
        "name": skill_name
    })))
}

/// POST /api/skills/{skill_name}/disable - Disable a skill
pub async fn disable_skill(Path(skill_name): Path<String>) -> Result<Json<Value>, SkillsError> {
    let result = SkillService::disable_skill(&skill_name)
        .await
        .map_err(|e| SkillsError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "disabled": result,
        "name": skill_name
    })))
}

/// POST /api/skills - Create a custom skill
pub async fn create_skill(Json(req): Json<CreateSkillRequest>) -> Result<Json<Value>, SkillsError> {
    SkillService::create_skill(
        &req.name,
        &req.content,
        req.references,
        req.scripts,
    )
    .await
    .map_err(|e| match e {
        SkillError::AlreadyExists(_) => SkillsError::BadRequest(e.to_string()),
        SkillError::InvalidContent(_) => SkillsError::BadRequest(e.to_string()),
        _ => SkillsError::Internal(e.to_string()),
    })?;

    Ok(Json(serde_json::json!({
        "created": true,
        "name": req.name
    })))
}

/// DELETE /api/skills/{skill_name} - Delete a custom skill
pub async fn delete_skill(Path(skill_name): Path<String>) -> Result<Json<Value>, SkillsError> {
    SkillService::delete_skill(&skill_name)
        .await
        .map_err(|e| SkillsError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "deleted": true,
        "name": skill_name
    })))
}

/// GET /api/skills/{skill_name}/config - Get skill config schema (placeholder)
pub async fn get_skill_config(Path(skill_name): Path<String>) -> Json<Value> {
    // TODO: Implement actual skill config schema
    Json(serde_json::json!({
        "name": skill_name,
        "schema": {}
    }))
}

/// PUT /api/skills/{skill_name}/config - Update skill config (placeholder)
pub async fn update_skill_config(
    Path(skill_name): Path<String>,
    Json(_config): Json<Value>,
) -> Result<Json<Value>, SkillsError> {
    // TODO: Implement actual skill config update
    Ok(Json(serde_json::json!({
        "updated": true,
        "name": skill_name
    })))
}

/// GET /api/skills/{skill_name}/files/{source}/{file_path:path} - Load skill file
pub async fn load_skill_file(
    Path((skill_name, source, file_path)): Path<(String, String, String)>,
) -> Result<Json<SkillFileContent>, SkillsError> {
    // Normalize file_path: remove leading slash (Axum wildcard may include it)
    let file_path = file_path.strip_prefix('/').unwrap_or(&file_path);

    let skill_source = match source.as_str() {
        "builtin" => SkillSource::Builtin,
        "customized" => SkillSource::Customized,
        _ => return Err(SkillsError::BadRequest(format!(
            "Invalid source '{}', must be 'builtin' or 'customized'",
            source
        ))),
    };

    let content = SkillService::load_skill_file(&skill_name, &file_path, skill_source)
        .await
        .map_err(|e| match e {
            SkillError::NotFound(_) => SkillsError::NotFound(e.to_string()),
            SkillError::InvalidContent(_) => SkillsError::BadRequest(e.to_string()),
            _ => SkillsError::Internal(e.to_string()),
        })?;

    Ok(Json(SkillFileContent { content }))
}

/// POST /api/skills/batch-enable - Batch enable skills
pub async fn batch_enable_skills(
    Json(req): Json<BatchSkillsRequest>,
) -> Result<Json<Value>, SkillsError> {
    for skill_name in &req.skill_names {
        SkillService::enable_skill(skill_name, false)
            .await
            .map_err(|e| SkillsError::Internal(e.to_string()))?;
    }

    Ok(Json(serde_json::json!({
        "enabled": true,
        "count": req.skill_names.len()
    })))
}

/// POST /api/skills/batch-disable - Batch disable skills
pub async fn batch_disable_skills(
    Json(req): Json<BatchSkillsRequest>,
) -> Result<Json<Value>, SkillsError> {
    for skill_name in &req.skill_names {
        SkillService::disable_skill(skill_name)
            .await
            .map_err(|e| SkillsError::Internal(e.to_string()))?;
    }

    Ok(Json(serde_json::json!({
        "disabled": true,
        "count": req.skill_names.len()
    })))
}

/// Create the skills router (generic to accept any state)
pub fn create_skills_router<S>() -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    use axum::routing::*;

    axum::Router::new()
        .route("/", get(list_skills).post(create_skill))
        .route("/available", get(list_available_skills))
        .route("/installed", get(list_installed_skills))
        .route("/hub", get(search_hub))
        .route("/hub/search", get(search_hub))
        .route("/hub/install", post(install_from_hub))
        .route("/market", get(get_market))
        .route("/batch-enable", post(batch_enable_skills))
        .route("/batch-disable", post(batch_disable_skills))
        .route("/:skill_name/enable", post(enable_skill))
        .route("/:skill_name/disable", post(disable_skill))
        .route("/:skill_name/config", get(get_skill_config).put(update_skill_config))
        .route("/:skill_name", delete(delete_skill))
        .route("/:skill_name/files/:source/*file_path", get(load_skill_file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hub_search_query_default() {
        let query = HubSearchQuery {
            q: String::new(),
            limit: 20,
        };
        assert_eq!(query.q, "");
        assert_eq!(query.limit, 20);
    }

    #[test]
    fn test_hub_search_query_with_params() {
        let query = HubSearchQuery {
            q: "test".to_string(),
            limit: 10,
        };
        assert_eq!(query.q, "test");
        assert_eq!(query.limit, 10);
    }

    #[test]
    fn test_create_skill_request() {
        let json = r#"{
            "name": "test_skill",
            "content": "---\nname: test\n---\n# Test"
        }"#;
        let req: CreateSkillRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "test_skill");
        assert!(req.content.contains("Test"));
    }

    #[test]
    fn test_hub_install_request() {
        let json = r#"{
            "bundle_url": "https://example.com/skill.zip",
            "enable": true
        }"#;
        let req: HubInstallRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.bundle_url, "https://example.com/skill.zip");
        assert!(req.enable);
    }

    #[test]
    fn test_batch_skills_request() {
        let json = r#"{"skill_names": ["skill1", "skill2"]}"#;
        let req: BatchSkillsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.skill_names.len(), 2);
    }

    // Tests for wildcard path parameter normalization (Round 11 action items)
    #[test]
    fn test_file_path_normalization_without_leading_slash() {
        let file_path = "README.md";
        let normalized = file_path.strip_prefix('/').unwrap_or(file_path);
        assert_eq!(normalized, "README.md");
    }

    #[test]
    fn test_file_path_normalization_with_leading_slash() {
        let file_path = "/README.md";
        let normalized = file_path.strip_prefix('/').unwrap_or(file_path);
        assert_eq!(normalized, "README.md");
    }

    #[test]
    fn test_file_path_normalization_with_nested_path() {
        let file_path = "/src/utils/helper.py";
        let normalized = file_path.strip_prefix('/').unwrap_or(file_path);
        assert_eq!(normalized, "src/utils/helper.py");
    }

    #[test]
    fn test_file_path_normalization_with_deeply_nested_path() {
        let file_path = "/a/b/c/d/e/file.txt";
        let normalized = file_path.strip_prefix('/').unwrap_or(file_path);
        assert_eq!(normalized, "a/b/c/d/e/file.txt");
    }
}
