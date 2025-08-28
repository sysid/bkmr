// src/application/services/mod.rs
pub mod action_service;
pub mod bookmark_service;
pub mod bookmark_service_impl;
pub mod tag_service;
pub mod tag_service_impl;
pub mod template_service;

// Re-export service interfaces
pub use action_service::ActionService;
pub use bookmark_service::BookmarkService;
pub use tag_service::TagService;
pub use template_service::TemplateService;

// Re-export service implementations
pub use action_service::ActionServiceImpl;
pub use bookmark_service_impl::BookmarkServiceImpl;
pub use tag_service_impl::TagServiceImpl;
pub use template_service::TemplateServiceImpl;
