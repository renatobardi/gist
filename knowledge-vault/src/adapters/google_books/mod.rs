use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

use crate::ports::external::{ExternalError, GoogleBooksMetadata, GoogleBooksPort};

const TIMEOUT_SECS: u64 = 10;

pub struct GoogleBooksClient {
    client: reqwest::Client,
    api_key: Option<String>,
}

impl GoogleBooksClient {
    pub fn build(api_key: Option<String>) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .user_agent("knowledge-vault/0.1 (renato.bardi@outlook.com)")
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()?;
        Ok(Self { client, api_key })
    }
}

// ---- Google Books API response shapes ----

#[derive(Debug, Deserialize)]
struct VolumeListResponse {
    #[serde(rename = "totalItems")]
    total_items: Option<u64>,
    items: Option<Vec<VolumeItem>>,
}

#[derive(Debug, Deserialize)]
struct VolumeItem {
    #[serde(rename = "volumeInfo")]
    volume_info: VolumeInfo,
}

#[derive(Debug, Deserialize)]
struct VolumeInfo {
    #[serde(rename = "pageCount")]
    page_count: Option<i32>,
    publisher: Option<String>,
    #[serde(rename = "averageRating")]
    average_rating: Option<f64>,
    #[serde(rename = "previewLink")]
    preview_link: Option<String>,
    #[serde(rename = "imageLinks")]
    image_links: Option<ImageLinks>,
}

#[derive(Debug, Deserialize)]
struct ImageLinks {
    thumbnail: Option<String>,
    #[serde(rename = "smallThumbnail")]
    small_thumbnail: Option<String>,
}

#[async_trait]
impl GoogleBooksPort for GoogleBooksClient {
    async fn fetch_by_isbn(&self, isbn: &str) -> Result<Option<GoogleBooksMetadata>, ExternalError> {
        let api_key = match &self.api_key {
            Some(k) => k,
            None => {
                return Err(ExternalError::SkippedOptional(
                    "KV_GOOGLE_BOOKS_API_KEY not set".to_string(),
                ))
            }
        };

        let url = format!(
            "https://www.googleapis.com/books/v1/volumes?q=isbn:{isbn}&key={api_key}"
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ExternalError::Transient(format!("Google Books timeout: {e}"))
                } else {
                    ExternalError::Transient(format!("Google Books request error: {e}"))
                }
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let msg = format!("Google Books API error {status}: {body}");
            return Err(
                if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS {
                    ExternalError::Permanent(msg)
                } else {
                    ExternalError::Transient(msg)
                },
            );
        }

        let volume_list: VolumeListResponse = resp.json().await.map_err(|e| {
            ExternalError::Transient(format!("failed to parse Google Books response: {e}"))
        })?;

        if volume_list.total_items.unwrap_or(0) == 0 {
            return Ok(None);
        }

        let volume_info = match volume_list.items.and_then(|items| items.into_iter().next()) {
            Some(item) => item.volume_info,
            None => return Ok(None),
        };

        let cover_image_url = volume_info
            .image_links
            .as_ref()
            .and_then(|links| links.thumbnail.clone().or_else(|| links.small_thumbnail.clone()))
            .or_else(|| {
                // Fallback to Open Library Covers API
                Some(format!(
                    "https://covers.openlibrary.org/b/isbn/{isbn}-M.jpg"
                ))
            });

        Ok(Some(GoogleBooksMetadata {
            cover_image_url,
            page_count: volume_info.page_count,
            publisher: volume_info.publisher,
            average_rating: volume_info.average_rating,
            preview_link: volume_info.preview_link,
        }))
    }
}
