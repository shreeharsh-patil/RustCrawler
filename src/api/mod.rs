pub mod agent_routes;
pub mod crawl_routes;
pub mod discovery_routes;
pub mod distributed_routes;
pub mod extraction_routes;
pub mod knowledge_routes;
pub mod models;
pub mod routes;

pub use routes::{app_router, router, AppState};
