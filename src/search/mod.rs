pub mod local_index;
pub mod models;
pub mod provider;
pub mod service;

pub use local_index::{IndexedDocument, LocalSearchIndex};
pub use models::{SearchMode, SearchRequest, SearchResponse, SearchResult, SearchResultSource};
pub use provider::{
    ExternalSearchProvider, MockSearchProvider, SearchProvider, SearchProviderRegistry,
};
pub use service::SearchService;
