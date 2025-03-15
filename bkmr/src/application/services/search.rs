/// Data Transfer Object for search parameters
#[derive(Debug, Clone, Default)]
pub struct SearchParamsDto {
    pub query: Option<String>,
    pub all_tags: Option<Vec<String>>,
    pub any_tags: Option<Vec<String>>,
    pub exclude_all_tags: Option<Vec<String>>,
    pub exclude_any_tags: Option<Vec<String>>,
    pub exact_tags: Option<Vec<String>>,
    pub sort_by_date: Option<bool>,
    pub sort_descending: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Data Transfer Object for search results
#[derive(Debug, Clone)]
pub struct SearchResultDto {
    pub bookmarks: Vec<crate::domain::bookmark::Bookmark>,
    pub total_count: usize,
}