use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;

use crate::ports::external::{BookMetadata, OpenLibraryPort};

pub struct OpenLibraryAdapter {
    client: reqwest::Client,
    base_url: String,
}

impl OpenLibraryAdapter {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build OpenLibrary HTTP client");
        Self {
            client,
            base_url: "https://openlibrary.org".to_string(),
        }
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build OpenLibrary HTTP client");
        Self {
            client,
            base_url: base_url.into(),
        }
    }
}

// Open Library /api/books response shape (jscmd=data)
#[derive(Debug, Deserialize)]
struct OlBookData {
    title: Option<String>,
    authors: Option<Vec<OlAuthor>>,
    description: Option<OlDescription>,
    subjects: Option<Vec<OlSubject>>,
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OlAuthor {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OlDescription {
    Simple(String),
    Object { value: String },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OlSubject {
    Simple(String),
    Object { name: String },
}

#[async_trait]
impl OpenLibraryPort for OpenLibraryAdapter {
    async fn fetch_metadata(&self, isbn: &str) -> Result<Option<BookMetadata>, String> {
        let url = format!(
            "{}/api/books?bibkeys=ISBN:{}&format=json&jscmd=data",
            self.base_url, isbn
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("OpenLibrary request failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "OpenLibrary returned status {}",
                response.status()
            ));
        }

        let map: HashMap<String, OlBookData> = response
            .json()
            .await
            .map_err(|e| format!("OpenLibrary response parse failed: {e}"))?;

        let book = match map.into_values().next() {
            Some(b) => b,
            None => return Ok(None),
        };

        let title = book.title.unwrap_or_default();

        let author = book
            .authors
            .unwrap_or_default()
            .into_iter()
            .filter_map(|a| a.name)
            .collect::<Vec<_>>()
            .join(", ");

        let description = match book.description {
            Some(OlDescription::Simple(s)) => s,
            Some(OlDescription::Object { value }) => value,
            None => String::new(),
        };

        let subjects = book
            .subjects
            .unwrap_or_default()
            .into_iter()
            .map(|s| match s {
                OlSubject::Simple(name) => name,
                OlSubject::Object { name } => name,
            })
            .take(20)
            .collect();

        let open_library_id = book.key;

        Ok(Some(BookMetadata {
            title,
            author,
            description,
            subjects,
            open_library_id,
        }))
    }
}
