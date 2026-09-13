pub mod info;
pub mod instagram;
pub mod youtube;

use axum::body::Body;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use std::path::PathBuf;
use tokio_util::io::ReaderStream;

use crate::error::AppError;

/// Streams a file response with Content-Disposition and schedules background cleanup
pub async fn stream_file_response(path: PathBuf, filename: String) -> Result<Response, AppError> {
    let file = tokio::fs::File::open(&path).await.map_err(|e| {
        AppError::NotFound(format!("Failed to open generated file: {}", e))
    })?;

    let file_len = file.metadata().await.ok().map(|m| m.len());
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let mime = mime_guess::from_path(&filename)
        .first_or_octet_stream()
        .to_string();

    let mut headers = HeaderMap::new();
    if let Ok(ct) = mime.parse() {
        headers.insert(CONTENT_TYPE, ct);
    }
    if let Some(len) = file_len {
        headers.insert(CONTENT_LENGTH, len.to_string().parse().unwrap());
    }
    if let Ok(cd) = format!("attachment; filename=\"{}\"", filename).parse() {
        headers.insert(CONTENT_DISPOSITION, cd);
    }

    // Background deletion after 15 minutes (900 seconds)
    let path_clone = path.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(900)).await;
        let _ = tokio::fs::remove_file(&path_clone).await;
    });

    Ok((headers, body).into_response())
}
