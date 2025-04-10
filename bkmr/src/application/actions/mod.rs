// src/application/actions/mod.rs
pub mod snippet_action;
pub mod text_action;
pub mod default_action;
pub mod uri_action;

pub use uri_action::UriAction;
pub use snippet_action::SnippetAction;
pub use text_action::TextAction;
pub use default_action::DefaultAction;