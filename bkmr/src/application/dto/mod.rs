pub mod bookmark_dto;
pub mod tag_dto;

// Re-export common DTOs for convenience
pub use bookmark_dto::{
    BookmarkCreateRequest, BookmarkUpdateRequest, BookmarkResponse,
    BookmarkSearchRequest, BookmarkSearchResponse, BookmarkListItem,
};
pub use tag_dto::{
    TagInfoResponse, TagOperationRequest, TagSuggestionResponse,
    TagMergeRequest, TagRenameRequest,
};