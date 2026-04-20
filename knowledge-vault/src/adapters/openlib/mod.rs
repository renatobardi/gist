use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::external::{OpenLibraryBook, OpenLibraryPort};

#[derive(Debug, Deserialize)]
struct SearchDoc {
    key: String,
    title: String,
    author_name: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    num_found: u64,
    docs: Vec<SearchDoc>,
}

pub struct OpenLibraryClient {
    client: reqwest::Client,
}

impl OpenLibraryClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("knowledge-vault/0.1 (renato.bardi@outlook.com)")
            .build()
            .expect("failed to build HTTP client");
        Self { client }
    }
}

impl Default for OpenLibraryClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OpenLibraryPort for OpenLibraryClient {
    async fn search_by_title(&self, title: &str) -> Result<Option<OpenLibraryBook>, String> {
        let url = reqwest::Url::parse_with_params(
            "https://openlibrary.org/search.json",
            &[
                ("title", title),
                ("limit", "1"),
                ("fields", "key,title,author_name"),
            ],
        )
        .map_err(|e| e.to_string())?
        .to_string();

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Open Library returned status {}", resp.status()));
        }

        let search: SearchResponse = resp.json().await.map_err(|e| e.to_string())?;

        if search.num_found == 0 || search.docs.is_empty() {
            return Ok(None);
        }

        let doc = &search.docs[0];
        let author = doc
            .author_name
            .as_ref()
            .and_then(|names| names.first().cloned())
            .unwrap_or_default();

        Ok(Some(OpenLibraryBook {
            open_library_id: doc.key.clone(),
            title: doc.title.clone(),
            author,
        }))
    }
}
