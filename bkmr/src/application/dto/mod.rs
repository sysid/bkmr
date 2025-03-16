pub mod bookmark_dto;
pub mod tag_dto;

// Re-export common DTOs for convenience
pub use bookmark_dto::{
    BookmarkCreateRequest, BookmarkListItem, BookmarkResponse, BookmarkSearchRequest,
    BookmarkSearchResponse, BookmarkUpdateRequest,
};
pub use tag_dto::{
    TagInfoResponse, TagMergeRequest, TagOperationRequest, TagRenameRequest, TagSuggestionResponse,
};
