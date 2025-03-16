/// Data Transfer Object for tag operations
#[derive(Debug, Clone)]
pub struct TagOperationDto {
    pub bookmark_ids: Vec<i32>,
    pub tags: Vec<String>,
    pub replace_existing: bool,
}
