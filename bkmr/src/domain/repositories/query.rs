// bkmr/src/domain/repositories/query.rs
use crate::domain::bookmark::Bookmark;
use crate::domain::tag::Tag;
use std::collections::HashSet;
use std::marker::PhantomData;

/*
   * The Specification pattern is a software design pattern used to define a business rule that can be
   * combined with other business rules to determine if an entity satisfies a criteria.
   *
   * 1. Specification Pattern
   * The specification pattern provides a powerful way to compose complex queries in a type-safe manner. It follows these principles:

   * Single Responsibility: Each specification handles one type of filtering criteria
   * Composability: Specifications can be combined using AND, OR, and NOT operators
   * Type Safety: The compiler ensures specifications are combined correctly
   * Encapsulation: Query logic is encapsulated in reusable components
*/

/// The Specification trait defines a predicate that determines if an entity matches criteria
pub trait Specification<T> {
    /// Check if an entity satisfies this specification
    fn is_satisfied_by(&self, entity: &T) -> bool;
}

// Add this implementation to allow using boxed specifications
impl<T> Specification<T> for Box<dyn Specification<T>> {
    fn is_satisfied_by(&self, entity: &T) -> bool {
        // Delegate to the inner specification
        (**self).is_satisfied_by(entity)
    }
}

/// Combines specifications with logical AND
pub struct AndSpecification<T, A, B>
where
    A: Specification<T>,
    B: Specification<T>,
{
    spec_a: A,
    spec_b: B,
    _marker: PhantomData<T>,
}

impl<T, A, B> AndSpecification<T, A, B>
where
    A: Specification<T>,
    B: Specification<T>,
{
    pub fn new(spec_a: A, spec_b: B) -> Self {
        Self {
            spec_a,
            spec_b,
            _marker: PhantomData,
        }
    }
}

impl<T, A, B> Specification<T> for AndSpecification<T, A, B>
where
    A: Specification<T>,
    B: Specification<T>,
{
    fn is_satisfied_by(&self, entity: &T) -> bool {
        self.spec_a.is_satisfied_by(entity) && self.spec_b.is_satisfied_by(entity)
    }
}

/// Combines specifications with logical OR
pub struct OrSpecification<T, A, B>
where
    A: Specification<T>,
    B: Specification<T>,
{
    spec_a: A,
    spec_b: B,
    _marker: PhantomData<T>,
}

impl<T, A, B> OrSpecification<T, A, B>
where
    A: Specification<T>,
    B: Specification<T>,
{
    pub fn new(spec_a: A, spec_b: B) -> Self {
        Self {
            spec_a,
            spec_b,
            _marker: PhantomData,
        }
    }
}

impl<T, A, B> Specification<T> for OrSpecification<T, A, B>
where
    A: Specification<T>,
    B: Specification<T>,
{
    fn is_satisfied_by(&self, entity: &T) -> bool {
        self.spec_a.is_satisfied_by(entity) || self.spec_b.is_satisfied_by(entity)
    }
}

/// Negates a specification
pub struct NotSpecification<T, S>
where
    S: Specification<T>,
{
    spec: S,
    _marker: PhantomData<T>,
}

impl<T, S> NotSpecification<T, S>
where
    S: Specification<T>,
{
    pub fn new(spec: S) -> Self {
        Self {
            spec,
            _marker: PhantomData,
        }
    }
}

impl<T, S> Specification<T> for NotSpecification<T, S>
where
    S: Specification<T>,
{
    fn is_satisfied_by(&self, entity: &T) -> bool {
        !self.spec.is_satisfied_by(entity)
    }
}

/// Specification for filtering bookmarks by tag (all tags must match)
pub struct AllTagsSpecification {
    tags: HashSet<Tag>,
}

impl AllTagsSpecification {
    pub fn new(tags: HashSet<Tag>) -> Self {
        Self { tags }
    }
}

impl Specification<Bookmark> for AllTagsSpecification {
    fn is_satisfied_by(&self, bookmark: &Bookmark) -> bool {
        bookmark.matches_all_tags(&self.tags)
    }
}

/// Specification for filtering bookmarks by tag (any tag may match)
pub struct AnyTagSpecification {
    tags: HashSet<Tag>,
}

impl AnyTagSpecification {
    pub fn new(tags: HashSet<Tag>) -> Self {
        Self { tags }
    }
}

impl Specification<Bookmark> for AnyTagSpecification {
    fn is_satisfied_by(&self, bookmark: &Bookmark) -> bool {
        bookmark.matches_any_tag(&self.tags)
    }
}

/// Specification for filtering bookmarks by exact tag match
pub struct ExactTagsSpecification {
    tags: HashSet<Tag>,
}

impl ExactTagsSpecification {
    pub fn new(tags: HashSet<Tag>) -> Self {
        Self { tags }
    }
}

impl Specification<Bookmark> for ExactTagsSpecification {
    fn is_satisfied_by(&self, bookmark: &Bookmark) -> bool {
        bookmark.matches_exact_tags(&self.tags)
    }
}

/// Specification for filtering bookmarks by text content
pub struct TextSearchSpecification {
    query: String,
}

impl TextSearchSpecification {
    pub fn new(query: String) -> Self {
        Self { query }
    }
}

impl Specification<Bookmark> for TextSearchSpecification {
    fn is_satisfied_by(&self, bookmark: &Bookmark) -> bool {
        if self.query.is_empty() {
            return true;
        }

        let query = self.query.to_lowercase();
        let content = format!(
            "{} {} {}",
            bookmark.title.to_lowercase(),
            bookmark.description.to_lowercase(),
            bookmark
                .tags
                .iter()
                .map(|t| t.value().to_lowercase())
                .collect::<Vec<_>>()
                .join(" ")
        );

        content.contains(&query)
    }
}

/// Extension trait to make combining specifications more readable
pub trait SpecificationExt<T>: Specification<T> {
    /// Combine with another specification using AND
    fn and<S: Specification<T>>(self, other: S) -> AndSpecification<T, Self, S>
    where
        Self: Sized,
    {
        AndSpecification::new(self, other)
    }

    /// Combine with another specification using OR
    fn or<S: Specification<T>>(self, other: S) -> OrSpecification<T, Self, S>
    where
        Self: Sized,
    {
        OrSpecification::new(self, other)
    }

    /// Negate this specification
    fn not(self) -> NotSpecification<T, Self>
    where
        Self: Sized,
    {
        NotSpecification::new(self)
    }
}

/// Implement SpecificationExt for all Specification implementors
impl<T, S> SpecificationExt<T> for S where S: Specification<T> {}

/// A query object that encapsulates specification and sorting
pub struct BookmarkQuery {
    pub specification: Option<Box<dyn Specification<Bookmark>>>,
    pub sort_by_date: Option<SortDirection>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl BookmarkQuery {
    pub fn new() -> Self {
        Self {
            specification: None,
            sort_by_date: None,
            limit: None,
            offset: None,
        }
    }

    pub fn with_specification<S>(mut self, spec: S) -> Self
    where
        S: Specification<Bookmark> + 'static,
    {
        self.specification = Some(Box::new(spec));
        self
    }
    // Add this method to the BookmarkQuery implementation
    pub fn with_specification_boxed(mut self, spec: Box<dyn Specification<Bookmark>>) -> Self {
        self.specification = Some(spec);
        self
    }

    pub fn with_sort_by_date(mut self, direction: SortDirection) -> Self {
        self.sort_by_date = Some(direction);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn matches(&self, bookmark: &Bookmark) -> bool {
        match &self.specification {
            Some(spec) => spec.is_satisfied_by(bookmark),
            None => true,
        }
    }
}

impl Default for BookmarkQuery {
    fn default() -> Self {
        Self::new()
    }
}

/// Sort direction enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::util::testing::init_test_env;

    #[test]
    fn test_and_specification() {
        let _ = init_test_env();

        // Create two simple specifications
        let spec1 = TextSearchSpecification::new("rust".to_string());
        let spec2 = TextSearchSpecification::new("programming".to_string());

        // Combine them with AND
        let and_spec = spec1.and(spec2);

        // Create a bookmark that matches both
        let tags = HashSet::new();
        let matching_bookmark = Bookmark::new(
            "https://example.com",
            "Rust Programming",
            "Learn Rust programming",
            tags.clone(),
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Create a bookmark that matches only one
        let partial_bookmark = Bookmark::new(
            "https://example.com",
            "Rust",
            "Learn Rust",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Test the specifications
        assert!(and_spec.is_satisfied_by(&matching_bookmark));
        assert!(!and_spec.is_satisfied_by(&partial_bookmark));
    }

    #[test]
    fn test_or_specification() {
        let _ = init_test_env();
        // Create two simple specifications
        let spec1 = TextSearchSpecification::new("rust".to_string());
        let spec2 = TextSearchSpecification::new("python".to_string());

        // Combine them with OR
        let or_spec = spec1.or(spec2);

        // Create bookmarks for testing
        let tags = HashSet::new();
        let rust_bookmark = Bookmark::new(
            "https://example.com",
            "Rust",
            "Learn Rust",
            tags.clone(),
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        let python_bookmark = Bookmark::new(
            "https://example.com",
            "Python",
            "Learn Python",
            tags.clone(),
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        let other_bookmark = Bookmark::new(
            "https://example.com",
            "JavaScript",
            "Learn JavaScript",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Test the specifications
        assert!(or_spec.is_satisfied_by(&rust_bookmark));
        assert!(or_spec.is_satisfied_by(&python_bookmark));
        assert!(!or_spec.is_satisfied_by(&other_bookmark));
    }

    #[test]
    fn test_not_specification() {
        let _ = init_test_env();
        let spec = TextSearchSpecification::new("rust".to_string());
        let not_spec = spec.not();

        let tags = HashSet::new();
        let rust_bookmark = Bookmark::new(
            "https://example.com",
            "Rust",
            "Learn Rust",
            tags.clone(),
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        let other_bookmark = Bookmark::new(
            "https://example.com",
            "Python",
            "Learn Python",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        assert!(!not_spec.is_satisfied_by(&rust_bookmark));
        assert!(not_spec.is_satisfied_by(&other_bookmark));
    }

    #[test]
    fn test_complex_specification() {
        let _ = init_test_env();
        // Create test tags
        let rust_tag = Tag::new("rust").unwrap();
        let programming_tag = Tag::new("programming").unwrap();

        // Create tag sets
        let mut rust_tags = HashSet::new();
        rust_tags.insert(rust_tag.clone());

        let mut programming_tags = HashSet::new();
        programming_tags.insert(programming_tag.clone());

        // Create specifications
        let has_rust_tag = AllTagsSpecification::new(rust_tags);
        let has_programming_tag = AllTagsSpecification::new(programming_tags);
        let about_web = TextSearchSpecification::new("web".to_string());

        // Create a complex specification: (has rust tag OR has programming tag) AND (is about web)
        let complex_spec = has_rust_tag.or(has_programming_tag).and(about_web);

        // Create test bookmarks
        let mut rust_web_tags = HashSet::new();
        rust_web_tags.insert(rust_tag.clone());

        let rust_web_bookmark = Bookmark::new(
            "https://example.com",
            "Rust Web",
            "Web development with Rust",
            rust_web_tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        let mut programming_web_tags = HashSet::new();
        programming_web_tags.insert(programming_tag.clone());

        let programming_web_bookmark = Bookmark::new(
            "https://example.com",
            "Web Programming",
            "Web development programming",
            programming_web_tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        let mut rust_tags = HashSet::new();
        rust_tags.insert(rust_tag.clone());

        let rust_bookmark = Bookmark::new(
            "https://example.com",
            "Rust",
            "Learn Rust",
            rust_tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Test the complex specification
        assert!(complex_spec.is_satisfied_by(&rust_web_bookmark));
        assert!(complex_spec.is_satisfied_by(&programming_web_bookmark));
        assert!(!complex_spec.is_satisfied_by(&rust_bookmark));
    }

    #[test]
    fn test_bookmark_query() {
        let _ = init_test_env();
        let tags = HashSet::new();
        let bookmark = Bookmark::new(
            "https://example.com",
            "Rust Programming",
            "Learn Rust programming",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Create query with text search specification
        let query = BookmarkQuery::new()
            .with_specification(TextSearchSpecification::new("rust".to_string()))
            .with_sort_by_date(SortDirection::Descending)
            .with_limit(10);

        assert!(query.matches(&bookmark));
        assert_eq!(query.sort_by_date, Some(SortDirection::Descending));
        assert_eq!(query.limit, Some(10));
    }
}
