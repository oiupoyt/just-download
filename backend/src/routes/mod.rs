pub mod info;
pub mod instagram;
pub mod youtube;

use axum::body::Body;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::io::ReaderStream;
use tracing::info;

use crate::error::AppError;

struct FileCleaner {
    path: PathBuf,
}

impl Drop for FileCleaner {
    fn drop(&mut self) {
        let path = self.path.clone();
        tokio::spawn(async move {
            let _ = tokio::fs::remove_file(&path).await;
            info!("Auto-cleaner unlinked served file immediately: {:?}", path);
        });
    }
}

/// Streams a file response with Content-Disposition and guarantees immediate cleanup
pub async fn stream_file_response(path: PathBuf, filename: String) -> Result<Response, AppError> {
    let file = tokio::fs::File::open(&path).await.map_err(|e| {
        AppError::NotFound(format!("Failed to open generated file: {}", e))
    })?;

    let file_len = file.metadata().await.ok().map(|m| m.len());
    let cleaner = Arc::new(FileCleaner {
        path: path.clone(),
    });

    let reader_stream = ReaderStream::new(file);
    // Bind the cleaner Arc to the stream so the file is unlinked immediately when stream ends or client disconnects
    let stream = reader_stream.map(move |chunk| {
        let _g = &cleaner;
        chunk
    });
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

    // Safety fallback: ensure deletion after 60 seconds even if stream somehow hangs
    let path_clone = path.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        let _ = tokio::fs::remove_file(&path_clone).await;
    });

    Ok((headers, body).into_response())
}
