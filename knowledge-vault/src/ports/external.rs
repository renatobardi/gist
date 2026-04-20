use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct OpenLibraryBook {
    pub open_library_id: String,
    pub title: String,
    pub author: String,
}

#[async_trait]
pub trait OpenLibraryPort: Send + Sync {
    async fn search_by_title(&self, title: &str) -> Result<Option<OpenLibraryBook>, String>;
}
