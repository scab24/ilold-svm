use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

use crate::state::{Annotation, AppState};

pub async fn list_annotations(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<Annotation>> {
    let annotations = state.annotations.read().unwrap();
    Json(annotations.clone())
}

pub async fn create_annotation(
    State(state): State<Arc<AppState>>,
    Json(mut annotation): Json<Annotation>,
) -> (StatusCode, Json<Annotation>) {
    if annotation.id.is_empty() {
        annotation.id = format!("ann-{}", state.annotations.read().unwrap().len());
    }
    let created = annotation.clone();
    state.annotations.write().unwrap().push(annotation);
    (StatusCode::CREATED, Json(created))
}

pub async fn update_annotation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(updated): Json<Annotation>,
) -> Result<Json<Annotation>, StatusCode> {
    let mut annotations = state.annotations.write().unwrap();
    let ann = annotations
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    ann.text = updated.text;
    ann.severity = updated.severity;
    ann.status = updated.status;
    Ok(Json(ann.clone()))
}

pub async fn delete_annotation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> StatusCode {
    let mut annotations = state.annotations.write().unwrap();
    let len_before = annotations.len();
    annotations.retain(|a| a.id != id);
    if annotations.len() < len_before {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}
