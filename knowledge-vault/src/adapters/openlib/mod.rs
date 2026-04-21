use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::external::{BookMetadata, ExternalError, OpenLibraryBook, OpenLibraryPort};

// ---- search_by_title response shapes ----

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

// ---- fetch_by_isbn response shapes ----

#[derive(Debug, Deserialize)]
struct OlBookResponse {
    title: Option<String>,
    #[serde(default)]
    authors: Vec<OlAuthorRef>,
    description: Option<OlDescription>,
    #[serde(default)]
    subjects: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OlAuthorRef {
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OlDescription {
    Simple(String),
    Object { value: String },
}

impl OlDescription {
    fn text(&self) -> &str {
        match self {
            OlDescription::Simple(s) => s.as_str(),
            OlDescription::Object { value } => value.as_str(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct OlAuthorResponse {
    name: Option<String>,
}

pub struct OpenLibraryClient {
    client: reqwest::Client,
}

impl OpenLibraryClient {
    pub fn build() -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .user_agent("knowledge-vault/0.1 (renato.bardi@outlook.com)")
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(Self { client })
    }
}

#[async_trait]
impl OpenLibraryPort for OpenLibraryClient {
    async fn search_by_title(&self, title: &str) -> Result<Option<OpenLibraryBook>, String> {
        let url = reqwest::Url::parse_with_params(
            "http://openlibrary.org/search.json",
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

    async fn fetch_by_isbn(&self, isbn: &str) -> Result<BookMetadata, ExternalError> {
        let url = format!("https://openlibrary.org/isbn/{isbn}.json");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ExternalError::Transient(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ExternalError::Permanent(format!(
                "ISBN {isbn} not found in OpenLibrary"
            )));
        }

        if !resp.status().is_success() {
            return Err(ExternalError::Transient(format!(
                "OpenLibrary returned HTTP {}",
                resp.status()
            )));
        }

        let book: OlBookResponse = resp
            .json()
            .await
            .map_err(|e| ExternalError::Transient(e.to_string()))?;

        let title = book.title.unwrap_or_else(|| "Unknown title".to_string());
        let description = book
            .description
            .as_ref()
            .map(|d| d.text().to_string())
            .unwrap_or_default();
        let subjects = book.subjects;

        let author = if let Some(author_ref) = book.authors.first() {
            if let Some(key) = &author_ref.key {
                let author_url = format!("https://openlibrary.org{key}.json");
                match self.client.get(&author_url).send().await {
                    Ok(r) if r.status().is_success() => r
                        .json::<OlAuthorResponse>()
                        .await
                        .ok()
                        .and_then(|a| a.name)
                        .unwrap_or_else(|| "Unknown author".to_string()),
                    _ => "Unknown author".to_string(),
                }
            } else {
                "Unknown author".to_string()
            }
        } else {
            "Unknown author".to_string()
        };

        Ok(BookMetadata {
            title,
            author,
            description,
            subjects,
        })
    }
}
