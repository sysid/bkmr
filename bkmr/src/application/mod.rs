// bkmr/src/application/mod.rs
pub mod error;
pub mod services;
pub mod templates;
mod actions;

// Re-export key services for easier imports
pub use services::bookmark_service_impl::BookmarkServiceImpl;
pub use services::tag_service_impl::TagServiceImpl;
pub use services::template_service::TemplateServiceImpl;
