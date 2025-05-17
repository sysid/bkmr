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
pub trait Specification<T>: std::fmt::Debug {
    /// Check if an entity satisfies this specification
    fn is_satisfied_by(&self, entity: &T) -> bool;
}

// Add this implementation to allow using boxed specifications
impl<T> Specification<T> for Box<dyn Specification<T>> {
    fn is_satisfied_by(&self, entity: &T) -> bool {
        // Delegate to the inner specification
        // *self dereferences the Box into a trait object.
        // **self gives a reference to the trait object (i.e., &dyn Specification<T>).
        // (**self).is_satisfied_by(entity)
        self.as_ref().is_satisfied_by(entity) // idiomatic way to call method on trait object
    }
}

/*
In the <xxx>Specification struct, the generic type parameter T is only used in constraints 
(where T: std::fmt::Debug) and method signatures, but it's not directly used as the type of 
any field in the struct. This creates a situation where the compiler might consider T unused, 
which can lead to issues:
1. Type Parameter Variance: Without PhantomData<T>, the compiler can't determine the proper variance for the unused type parameter.
2. Lifetime Inference: For type parameters that include lifetimes, the compiler needs to know how they're used within the struct.
3. Drop Check Semantics: The compiler needs to know if the generic type's destructor should be considered when dropping the struct.

It makes the connection between the struct and the type parameter explicit and helps the compiler 
understand how the type is conceptually used, even if it doesn't appear in a field.
 */

/// Combines specifications with logical AND
#[derive(Debug)]
pub struct AndSpecification<T, A, B>
where
    T: std::fmt::Debug,
    A: Specification<T>,
    B: Specification<T>,
{
    spec_a: A,
    spec_b: B,
    _marker: PhantomData<T>,
}

impl<T, A, B> AndSpecification<T, A, B>
where
    T: std::fmt::Debug,
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
    T: std::fmt::Debug,
    A: Specification<T>,
    B: Specification<T>,
{
    fn is_satisfied_by(&self, entity: &T) -> bool {
        self.spec_a.is_satisfied_by(entity) && self.spec_b.is_satisfied_by(entity)
    }
}

/// Combines specifications with logical OR
#[derive(Debug)]
pub struct OrSpecification<T, A, B>
where
    T: std::fmt::Debug,
    A: Specification<T>,
    B: Specification<T>,
{
    spec_a: A,
    spec_b: B,
    _marker: PhantomData<T>,
}

impl<T, A, B> OrSpecification<T, A, B>
where
    T: std::fmt::Debug,
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
    T: std::fmt::Debug,
    A: Specification<T>,
    B: Specification<T>,
{
    fn is_satisfied_by(&self, entity: &T) -> bool {
        self.spec_a.is_satisfied_by(entity) || self.spec_b.is_satisfied_by(entity)
    }
}

/// Negates a specification
#[derive(Debug)]
pub struct NotSpecification<T, S>
where
    T: std::fmt::Debug,
    S: Specification<T>,
{
    spec: S,
    _marker: PhantomData<T>,
}

impl<T, S> NotSpecification<T, S>
where
    T: std::fmt::Debug,
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
    T: std::fmt::Debug,
    S: Specification<T>,
{
    fn is_satisfied_by(&self, entity: &T) -> bool {
        !self.spec.is_satisfied_by(entity)
    }
}

/// Specification for filtering bookmarks by tag (all tags must match)
#[derive(Debug)]
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
#[derive(Debug)]
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
#[derive(Debug)]
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
#[derive(Debug)]
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
pub trait SpecificationExt<T: std::fmt::Debug>: Specification<T> {
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
impl<T, S> SpecificationExt<T> for S
where
    S: Specification<T>,
    T: std::fmt::Debug,
{
}

/// A query object that encapsulates specification and sorting
#[derive(Debug)]
pub struct BookmarkQuery {
    pub specification: Option<Box<dyn Specification<Bookmark>>>,
    pub sort_by_date: Option<SortDirection>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,

    // New fields for direct filter support
    pub text_query: Option<String>,
    pub tags_exact: Option<HashSet<Tag>>,
    pub tags_all: Option<HashSet<Tag>>,
    pub tags_all_not: Option<HashSet<Tag>>,
    pub tags_any: Option<HashSet<Tag>>,
    pub tags_any_not: Option<HashSet<Tag>>,
    pub tags_prefix: Option<HashSet<Tag>>,
}

impl BookmarkQuery {
    pub fn new() -> Self {
        Self {
            specification: None,
            sort_by_date: None,
            limit: None,
            offset: None,
            text_query: None,
            tags_exact: None,
            tags_all: None,
            tags_all_not: None,
            tags_any: None,
            tags_any_not: None,
            tags_prefix: None,
        }
    }

    // Enhanced builder methods
    pub fn with_text_query(mut self, query: Option<&str>) -> Self {
        self.text_query = query.map(|s| s.to_string());
        self
    }

    pub fn with_tags_exact(mut self, tags: Option<&HashSet<Tag>>) -> Self {
        self.tags_exact = tags.cloned();
        self
    }

    pub fn with_tags_all(mut self, tags: Option<&HashSet<Tag>>) -> Self {
        self.tags_all = tags.cloned();
        self
    }

    pub fn with_tags_all_not(mut self, tags: Option<&HashSet<Tag>>) -> Self {
        self.tags_all_not = tags.cloned();
        self
    }

    pub fn with_tags_any(mut self, tags: Option<&HashSet<Tag>>) -> Self {
        self.tags_any = tags.cloned();
        self
    }

    pub fn with_tags_any_not(mut self, tags: Option<&HashSet<Tag>>) -> Self {
        self.tags_any_not = tags.cloned();
        self
    }

    pub fn with_tags_prefix(mut self, tags: Option<&HashSet<Tag>>) -> Self {
        self.tags_prefix = tags.cloned();
        self
    }

    /// takes any type that implements Specification<Bookmark> and boxes it, storing it in the query.
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

    pub fn with_limit(mut self, limit: Option<usize>) -> Self {
        self.limit = limit;
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

    // Add a new method to apply all filters to a collection of bookmarks
    pub fn apply_non_text_filters(&self, bookmarks: &[Bookmark]) -> Vec<Bookmark> {
        let mut filtered = bookmarks.to_vec();

        // Apply specification if present
        if let Some(spec) = &self.specification {
            filtered.retain(|bookmark| spec.is_satisfied_by(bookmark));
        }

        // Apply exact tag matching
        if let Some(tags) = &self.tags_exact {
            if !tags.is_empty() {
                filtered.retain(|bookmark| bookmark.matches_exact_tags(tags));
            }
        }

        // Apply all tags filter
        if let Some(tags) = &self.tags_all {
            if !tags.is_empty() {
                filtered.retain(|bookmark| bookmark.matches_all_tags(tags));
            }
        }

        // Apply all-not tags filter
        if let Some(tags) = &self.tags_all_not {
            if !tags.is_empty() {
                filtered.retain(|bookmark| !bookmark.matches_all_tags(tags));
            }
        }

        // Apply any tags filter
        if let Some(tags) = &self.tags_any {
            if !tags.is_empty() {
                filtered.retain(|bookmark| bookmark.matches_any_tag(tags));
            }
        }

        // Apply any-not tags filter
        if let Some(tags) = &self.tags_any_not {
            if !tags.is_empty() {
                filtered.retain(|bookmark| !bookmark.matches_any_tag(tags));
            }
        }

        // Apply tag prefix filtering
        if let Some(prefixes) = &self.tags_prefix {
            if !prefixes.is_empty() {
                filtered.retain(|bookmark| {
                    prefixes.iter().any(|prefix| {
                        let prefix_str = prefix.value();
                        bookmark
                            .tags
                            .iter()
                            .any(|tag| tag.value().starts_with(prefix_str))
                    })
                });
            }
        }

        // Apply sorting if specified
        if let Some(direction) = &self.sort_by_date {
            match direction {
                SortDirection::Ascending => {
                    filtered.sort_by(|a, b| a.updated_at.cmp(&b.updated_at));
                }
                SortDirection::Descending => {
                    filtered.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                }
            }
        }

        // Apply limit and offset
        if let Some(offset) = self.offset {
            if offset < filtered.len() {
                filtered = filtered.into_iter().skip(offset).collect();
            } else {
                filtered.clear();
            }
        }

        if let Some(limit) = self.limit {
            filtered.truncate(limit);
        }

        filtered
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
            .with_limit(Some(10));

        assert!(query.matches(&bookmark));
        assert_eq!(query.sort_by_date, Some(SortDirection::Descending));
        assert_eq!(query.limit, Some(10));
    }

    #[test]
    fn test_apply_non_text_filters() {
        let _ = init_test_env();

        // Create test bookmarks with various properties
        let app_state = AppState::read_global();
        let embedder = &*app_state.context.embedder;

        // Create some test bookmarks
        let now = chrono::Utc::now();
        let one_day_ago = now - chrono::Duration::days(1);
        let two_days_ago = now - chrono::Duration::days(2);

        // Bookmark 1: has tags "rust", "programming"
        let mut tags1 = HashSet::new();
        tags1.insert(Tag::new("rust").unwrap());
        tags1.insert(Tag::new("programming").unwrap());
        let mut bookmark1 = Bookmark::new(
            "https://example.com/rust",
            "Rust Programming",
            "Learn Rust programming",
            tags1,
            embedder,
        )
        .unwrap();
        bookmark1.id = Some(1);
        bookmark1.updated_at = now;

        // Bookmark 2: has tags "python", "programming", "web"
        let mut tags2 = HashSet::new();
        tags2.insert(Tag::new("python").unwrap());
        tags2.insert(Tag::new("programming").unwrap());
        tags2.insert(Tag::new("web").unwrap());
        let mut bookmark2 = Bookmark::new(
            "https://example.com/python",
            "Python Web Development",
            "Learn Python web development",
            tags2,
            embedder,
        )
        .unwrap();
        bookmark2.id = Some(2);
        bookmark2.updated_at = one_day_ago;

        // Bookmark 3: has tags "java", "enterprise"
        let mut tags3 = HashSet::new();
        tags3.insert(Tag::new("java").unwrap());
        tags3.insert(Tag::new("enterprise").unwrap());
        let mut bookmark3 = Bookmark::new(
            "https://example.com/java",
            "Java Enterprise",
            "Enterprise Java development",
            tags3,
            embedder,
        )
        .unwrap();
        bookmark3.id = Some(3);
        bookmark3.updated_at = two_days_ago;

        // Create a collection of bookmarks
        let bookmarks = vec![bookmark1.clone(), bookmark2.clone(), bookmark3.clone()];

        // Test 1: Filter by exact tags
        let mut exact_tags = HashSet::new();
        exact_tags.insert(Tag::new("rust").unwrap());
        exact_tags.insert(Tag::new("programming").unwrap());

        let query1 = BookmarkQuery::new().with_tags_exact(Some(&exact_tags));
        let results1 = query1.apply_non_text_filters(&bookmarks);

        assert_eq!(results1.len(), 1);
        assert_eq!(results1[0].id, Some(1));

        // Test 2: Filter by all tags
        let mut all_tags = HashSet::new();
        all_tags.insert(Tag::new("programming").unwrap());

        let query2 = BookmarkQuery::new().with_tags_all(Some(&all_tags));
        let results2 = query2.apply_non_text_filters(&bookmarks);

        assert_eq!(results2.len(), 2);
        assert!(results2.iter().any(|b| b.id == Some(1)));
        assert!(results2.iter().any(|b| b.id == Some(2)));

        // Test 3: Filter by any tags
        let mut any_tags = HashSet::new();
        any_tags.insert(Tag::new("enterprise").unwrap());
        any_tags.insert(Tag::new("rust").unwrap());

        let query3 = BookmarkQuery::new().with_tags_any(Some(&any_tags));
        let results3 = query3.apply_non_text_filters(&bookmarks);

        assert_eq!(results3.len(), 2);
        assert!(results3.iter().any(|b| b.id == Some(1)));
        assert!(results3.iter().any(|b| b.id == Some(3)));

        // Test 4: Filter by tags all not
        let mut all_not_tags = HashSet::new();
        all_not_tags.insert(Tag::new("programming").unwrap());

        let query4 = BookmarkQuery::new().with_tags_all_not(Some(&all_not_tags));
        let results4 = query4.apply_non_text_filters(&bookmarks);

        assert_eq!(results4.len(), 1);
        assert_eq!(results4[0].id, Some(3));

        // Test 5: Filter by tags any not
        let mut any_not_tags = HashSet::new();
        any_not_tags.insert(Tag::new("web").unwrap());
        any_not_tags.insert(Tag::new("enterprise").unwrap());

        let query5 = BookmarkQuery::new().with_tags_any_not(Some(&any_not_tags));
        let results5 = query5.apply_non_text_filters(&bookmarks);

        assert_eq!(results5.len(), 1);
        assert_eq!(results5[0].id, Some(1));

        // Test 6: Filter by tag prefix
        let mut prefix_tags = HashSet::new();
        prefix_tags.insert(Tag::new("pro").unwrap()); // Should match "programming"

        let query6 = BookmarkQuery::new().with_tags_prefix(Some(&prefix_tags));
        let results6 = query6.apply_non_text_filters(&bookmarks);

        assert_eq!(results6.len(), 2);
        assert!(results6.iter().any(|b| b.id == Some(1)));
        assert!(results6.iter().any(|b| b.id == Some(2)));

        // Test 7: Sorting by date (ascending)
        let query7 = BookmarkQuery::new().with_sort_by_date(SortDirection::Ascending);
        let results7 = query7.apply_non_text_filters(&bookmarks);

        assert_eq!(results7.len(), 3);
        assert_eq!(results7[0].id, Some(3)); // Oldest first
        assert_eq!(results7[1].id, Some(2));
        assert_eq!(results7[2].id, Some(1)); // Newest last

        // Test 8: Sorting by date (descending)
        let query8 = BookmarkQuery::new().with_sort_by_date(SortDirection::Descending);
        let results8 = query8.apply_non_text_filters(&bookmarks);

        assert_eq!(results8.len(), 3);
        assert_eq!(results8[0].id, Some(1)); // Newest first
        assert_eq!(results8[1].id, Some(2));
        assert_eq!(results8[2].id, Some(3)); // Oldest last

        // Test 9: Limit results
        let query9 = BookmarkQuery::new().with_limit(Some(2));
        let results9 = query9.apply_non_text_filters(&bookmarks);

        assert_eq!(results9.len(), 2);

        // Test 10: Offset results
        let query10 = BookmarkQuery::new().with_offset(1);
        let results10 = query10.apply_non_text_filters(&bookmarks);

        assert_eq!(results10.len(), 2);
        assert!(results10.iter().any(|b| b.id == Some(2)));
        assert!(results10.iter().any(|b| b.id == Some(3)));

        // Test 11: Combining multiple filters
        let combined_query = BookmarkQuery::new()
            .with_tags_all(Some(&all_tags))
            .with_tags_any_not(Some(&any_not_tags))
            .with_sort_by_date(SortDirection::Descending)
            .with_limit(Some(1));

        let combined_results = combined_query.apply_non_text_filters(&bookmarks);

        assert_eq!(combined_results.len(), 1);
        assert_eq!(combined_results[0].id, Some(1));

        // Test 12: Empty tags set
        let empty_tags = HashSet::new();
        let query12 = BookmarkQuery::new().with_tags_all(Some(&empty_tags));
        let results12 = query12.apply_non_text_filters(&bookmarks);

        assert_eq!(
            results12.len(),
            3,
            "Empty tag set should not filter anything"
        );

        // Test 13: Specification filter
        let spec = TextSearchSpecification::new("rust".to_string());
        let query13 = BookmarkQuery::new().with_specification(spec);
        let results13 = query13.apply_non_text_filters(&bookmarks);

        assert_eq!(results13.len(), 1);
        assert_eq!(results13[0].id, Some(1));
    }

    #[test]
    fn test_apply_non_text_filters_empty_bookmarks() {
        let _ = init_test_env();

        // Create an empty collection of bookmarks
        let bookmarks: Vec<Bookmark> = Vec::new();

        // Create a query with various filters
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let query = BookmarkQuery::new()
            .with_tags_all(Some(&tags))
            .with_sort_by_date(SortDirection::Descending)
            .with_limit(Some(10));

        // Apply filters to empty collection
        let results = query.apply_non_text_filters(&bookmarks);

        // Should still be empty
        assert!(
            results.is_empty(),
            "Filtering empty collection should return empty results"
        );
    }

    #[test]
    fn test_specification_boxed() {
        let _ = init_test_env();

        // Create test bookmarks
        let tags = HashSet::new();
        let app_state = AppState::read_global();
        let embedder = &*app_state.context.embedder;

        let bookmark = Bookmark::new(
            "https://example.com",
            "Test Bookmark",
            "This is a test",
            tags,
            embedder,
        )
        .unwrap();

        // Create a specification
        let spec = TextSearchSpecification::new("test".to_string());

        // Convert to boxed specification
        let boxed_spec: Box<dyn Specification<Bookmark>> = Box::new(spec);

        // Create a query with the boxed specification
        let query = BookmarkQuery::new().with_specification_boxed(boxed_spec);

        // Test that the specification works through the query
        assert!(query.matches(&bookmark), "Boxed specification should match");

        // Create a specification that shouldn't match
        let non_matching_spec = TextSearchSpecification::new("nonexistent".to_string());
        let boxed_non_matching: Box<dyn Specification<Bookmark>> = Box::new(non_matching_spec);

        // Create a query with the non-matching specification
        let non_matching_query = BookmarkQuery::new().with_specification_boxed(boxed_non_matching);

        // Test that it correctly doesn't match
        assert!(
            !non_matching_query.matches(&bookmark),
            "Non-matching boxed specification should not match"
        );
    }
}
