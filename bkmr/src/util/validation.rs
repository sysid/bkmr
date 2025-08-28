use crate::application::error::{ApplicationError, ApplicationResult};
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::repositories::repository::BookmarkRepository;

/// Utility for common validation patterns across services
pub struct ValidationHelper;

impl ValidationHelper {
    /// Validates that a bookmark ID is positive (> 0)
    ///
    /// # Arguments
    /// * `id` - The bookmark ID to validate
    ///
    /// # Returns
    /// * `Ok(())` - If the ID is valid
    /// * `Err(ApplicationError::Validation)` - If the ID is invalid (≤ 0)
    pub fn validate_bookmark_id(id: i32) -> ApplicationResult<()> {
        if id <= 0 {
            return Err(ApplicationError::Validation(format!(
                "Invalid bookmark ID: {}",
                id
            )));
        }
        Ok(())
    }

    /// Validates bookmark ID and retrieves the bookmark from repository, returning an error if not found
    ///
    /// # Arguments
    /// * `id` - The bookmark ID to validate and fetch
    /// * `repository` - The repository to fetch from
    ///
    /// # Returns
    /// * `Ok(Bookmark)` - If the ID is valid and bookmark exists
    /// * `Err(ApplicationError)` - If the ID is invalid or bookmark not found
    pub fn validate_and_get_bookmark<R: BookmarkRepository>(
        id: i32,
        repository: &R,
    ) -> ApplicationResult<Bookmark> {
        Self::validate_bookmark_id(id)?;

        repository
            .get_by_id(id)?
            .ok_or(ApplicationError::BookmarkNotFound(id))
    }

    /// Domain-level validation and retrieval (for domain services like ActionService)
    ///
    /// # Arguments
    /// * `id` - The bookmark ID to validate and fetch
    /// * `repository` - The repository to fetch from
    ///
    /// # Returns
    /// * `Ok(Bookmark)` - If the ID is valid and bookmark exists
    /// * `Err(DomainError)` - If the ID is invalid or bookmark not found
    pub fn validate_and_get_bookmark_domain<R: BookmarkRepository>(
        id: i32,
        repository: &R,
    ) -> DomainResult<Bookmark> {
        // For domain level, we use a simpler validation (no specific ID validation,
        // just rely on repository behavior)
        repository
            .get_by_id(id)?
            .ok_or_else(|| DomainError::BookmarkNotFound(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_positive_bookmark_id_when_validate_then_returns_ok() {
        let result = ValidationHelper::validate_bookmark_id(1);
        assert!(result.is_ok());
    }

    #[test]
    fn given_zero_bookmark_id_when_validate_then_returns_error() {
        let result = ValidationHelper::validate_bookmark_id(0);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid bookmark ID: 0"));
    }

    #[test]
    fn given_negative_bookmark_id_when_validate_then_returns_error() {
        let result = ValidationHelper::validate_bookmark_id(-1);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid bookmark ID: -1"));
    }
}
