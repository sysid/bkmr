#[cfg(test)]
mod tests {
    
    use crate::domain::bookmark::Bookmark;
    use crate::domain::tag::Tag;
    use std::collections::HashSet;
    use crate::application::dto::BookmarkSearchRequest;
    use crate::application::services::bookmark_application_service::BookmarkApplicationService;
    use crate::domain::repositories::bookmark_repository::BookmarkRepository;
    use crate::infrastructure::repositories::in_memory::bookmark_repository::InMemoryBookmarkRepository;

    fn create_service() -> BookmarkApplicationService<InMemoryBookmarkRepository> {
        let repo = InMemoryBookmarkRepository::new();
        BookmarkApplicationService::new(repo)
    }

    /// Helper that creates and adds a bookmark to the underlying repository
    /// directly, bypassing the service if needed for setup convenience.
    fn add_bookmark(
        service: &BookmarkApplicationService<InMemoryBookmarkRepository>,
        url: &str,
        title: &str,
        description: &str,
        tags: &[&str],
    ) -> i32 {
        // Convert string slice tags to domain tags
        let mut tagset = HashSet::new();
        for t in tags {
            tagset.insert(Tag::new(t).unwrap());
        }

        // Build a new domain bookmark
        let mut bookmark = Bookmark::new(url, title, description, tagset).unwrap();

        // Insert it via the repository to set an ID
        service.repository.add(&mut bookmark).unwrap();
        bookmark.id().unwrap()
    }

    #[test]
    fn test_search_text_only() {
        let service = create_service();

        // Add various bookmarks
        add_bookmark(&service, "https://rust-lang.org", "Rust Book", "Rust language docs", &["tag1"]);
        add_bookmark(&service, "https://python.org", "Python Book", "Python language docs", &["tag2"]);
        add_bookmark(&service, "https://other.com", "Other Title", "Unrelated desc", &["tag3"]);

        // Search for 'rust'
        let request = BookmarkSearchRequest {
            query: Some("rust".to_string()),
            ..Default::default()
        };
        let response = service.search_bookmarks(request).unwrap();
        assert_eq!(response.total_count, 1);
        assert_eq!(response.bookmarks[0].url, "https://rust-lang.org");

        // Search for 'language'
        let request = BookmarkSearchRequest {
            query: Some("language".to_string()),
            ..Default::default()
        };
        let response = service.search_bookmarks(request).unwrap();
        assert_eq!(response.total_count, 2); // Rust and Python
    }

    #[test]
    fn test_search_all_tags() {
        let service = create_service();

        // Create a single bookmark that has both tagX and tagY
        let both_id = add_bookmark(&service, "https://both.com", "Both", "Has both tags", &["tagX", "tagY"]);
        // Another that has only tagX
        let _x_id = add_bookmark(&service, "https://onlyx.com", "OnlyX", "Has tagX", &["tagX"]);
        // Another that has only tagY
        let _y_id = add_bookmark(&service, "https://onlyy.com", "OnlyY", "Has tagY", &["tagY"]);

        // We want bookmarks that contain both tagX and tagY
        let request = BookmarkSearchRequest {
            all_tags: Some(vec!["tagX".to_string(), "tagY".to_string()]),
            ..Default::default()
        };
        let response = service.search_bookmarks(request).unwrap();

        assert_eq!(response.total_count, 1);
        let found = &response.bookmarks[0];
        assert_eq!(found.id, Some(both_id));
        assert_eq!(found.url, "https://both.com");
    }

    #[test]
    fn test_search_any_tags() {
        let service = create_service();

        // Tag combos
        let _a_id = add_bookmark(&service, "https://a.com", "A", "Desc", &["rust", "cpp"]);
        let _b_id = add_bookmark(&service, "https://b.com", "B", "Desc", &["python"]);
        let _c_id = add_bookmark(&service, "https://c.com", "C", "Desc", &["java", "rust"]);

        // ANY tags = [rust, python]
        // This means we want all bookmarks that have either rust OR python
        let request = BookmarkSearchRequest {
            any_tags: Some(vec!["rust".into(), "python".into()]),
            ..Default::default()
        };
        let response = service.search_bookmarks(request).unwrap();
        // Should find a.com (rust,cpp), b.com (python), c.com (java, rust) => total 3
        assert_eq!(response.total_count, 3);

        // ANY tags = [cpp], only a.com qualifies
        let request = BookmarkSearchRequest {
            any_tags: Some(vec!["cpp".into()]),
            ..Default::default()
        };
        let response = service.search_bookmarks(request).unwrap();
        assert_eq!(response.total_count, 1);
        assert_eq!(response.bookmarks[0].url, "https://a.com");
    }

    #[test]
    fn test_exclude_all_tags() {
        let service = create_service();

        // Bookmarks with various tags
        add_bookmark(&service, "https://ex1.com", "Ex1", "Desc", &["tag1", "tag2"]);
        add_bookmark(&service, "https://ex2.com", "Ex2", "Desc", &["tag1"]);
        add_bookmark(&service, "https://ex3.com", "Ex3", "Desc", &["tag2"]);
        add_bookmark(&service, "https://ex4.com", "Ex4", "Desc", &["tag3"]);

        // Exclude all tags => [tag1] means we only want bookmarks that do NOT have all of 'tag1'
        // That excludes those which have tag1 as a subset, but careful: `exclude_all_tags` means
        // a NOT (AllTagsSpecification).
        let request = BookmarkSearchRequest {
            exclude_all_tags: Some(vec!["tag1".to_string()]),
            ..Default::default()
        };
        let response = service.search_bookmarks(request).unwrap();

        // "ex1.com" has tag1 and tag2 => it DOES have 'tag1'
        // "ex2.com" has tag1 => also has 'tag1'
        // "ex3.com" has tag2 => does NOT have 'tag1'
        // "ex4.com" has tag3 => does NOT have 'tag1'
        // So ex3, ex4 remain => total_count=2
        assert_eq!(response.total_count, 2);
        let found_urls: Vec<_> = response.bookmarks.iter().map(|b| b.url.clone()).collect();
        assert!(found_urls.contains(&"https://ex3.com".to_string()));
        assert!(found_urls.contains(&"https://ex4.com".to_string()));
    }

    #[test]
    fn test_exclude_any_tags() {
        let service = create_service();

        // Bookmarks
        add_bookmark(&service, "https://x.com", "X", "Desc", &["java"]);
        add_bookmark(&service, "https://y.com", "Y", "Desc", &["java", "python"]);
        add_bookmark(&service, "https://z.com", "Z", "Desc", &["rust", "go"]);

        // Exclude any => [java], means NOT (AnyTagSpecification with 'java')
        // So we exclude bookmarks that have 'java' at all
        let request = BookmarkSearchRequest {
            exclude_any_tags: Some(vec!["java".into()]),
            ..Default::default()
        };
        let resp = service.search_bookmarks(request).unwrap();
        // X has 'java', Y has 'java' and 'python', Z has rust, go => only Z remains
        assert_eq!(resp.total_count, 1);
        assert_eq!(resp.bookmarks[0].url, "https://z.com");
    }

    #[test]
    fn test_exact_tags() {
        let service = create_service();

        // Bookmarks
        let _b1 = add_bookmark(&service, "https://b1.com", "B1", "Desc", &["rust", "lang"]);
        let _b2 = add_bookmark(&service, "https://b2.com", "B2", "Desc", &["rust", "lang", "extra"]);
        let _b3 = add_bookmark(&service, "https://b3.com", "B3", "Desc", &["rust"]);

        // EXACT => [rust, lang] means we only want bookmarks whose tags are precisely {rust, lang}
        let request = BookmarkSearchRequest {
            exact_tags: Some(vec!["rust".into(), "lang".into()]),
            ..Default::default()
        };
        let response = service.search_bookmarks(request).unwrap();
        assert_eq!(response.total_count, 1);
        assert_eq!(response.bookmarks[0].url, "https://b1.com");

        // EXACT => [rust], only b3 qualifies
        let request = BookmarkSearchRequest {
            exact_tags: Some(vec!["rust".into()]),
            ..Default::default()
        };
        let response = service.search_bookmarks(request).unwrap();
        assert_eq!(response.total_count, 1);
        assert_eq!(response.bookmarks[0].url, "https://b3.com");
    }

    #[test]
    fn test_sort_by_date() {
        let service = create_service();

        // We'll just rely on creation order for updated_at
        let id1 = add_bookmark(&service, "https://first.com", "First", "Desc", &[]);
        std::thread::sleep(std::time::Duration::from_millis(5));
        let id2 = add_bookmark(&service, "https://second.com", "Second", "Desc", &[]);
        std::thread::sleep(std::time::Duration::from_millis(5));
        let id3 = add_bookmark(&service, "https://third.com", "Third", "Desc", &[]);

        // Sort descending => third, second, first
        let request = BookmarkSearchRequest {
            sort_by_date: Some(true),
            sort_descending: Some(true),
            ..Default::default()
        };
        let resp = service.search_bookmarks(request).unwrap();
        let found = &resp.bookmarks;
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].id, Some(id3));
        assert_eq!(found[1].id, Some(id2));
        assert_eq!(found[2].id, Some(id1));

        // Sort ascending => first, second, third
        let request = BookmarkSearchRequest {
            sort_by_date: Some(true),
            sort_descending: Some(false),
            ..Default::default()
        };
        let resp = service.search_bookmarks(request).unwrap();
        let found = &resp.bookmarks;
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].id, Some(id1));
        assert_eq!(found[1].id, Some(id2));
        assert_eq!(found[2].id, Some(id3));
    }

    #[test]
    fn test_pagination() {
        let service = create_service();

        // Insert multiple bookmarks
        for i in 1..=5 {
            let url = format!("https://{}.com", i);
            let title = format!("Title {}", i);
            add_bookmark(&service, &url, &title, "Desc", &[]);
        }

        // Let's do limit=2, offset=1
        let request = BookmarkSearchRequest {
            limit: Some(2),
            offset: Some(1),
            ..Default::default()
        };
        let resp = service.search_bookmarks(request).unwrap();
        assert_eq!(resp.total_count, 2); // total in memory
        assert_eq!(resp.bookmarks.len(), 2); // we get 2 in the 'page'
        // by default they're sorted ascending by ID or creation order if no sort param
        // so we expect items #2 and #3 if the default order is ID ascending
    }

    #[test]
    fn test_complex_search() {
        let service = create_service();

        // A variety of tags
        add_bookmark(&service, "https://b1.com", "Rust Web", "Rust+Web dev", &["rust", "web"]);
        add_bookmark(&service, "https://b2.com", "Rust Gaming", "Rust+Game dev", &["rust", "game"]);
        add_bookmark(&service, "https://b3.com", "Python Web", "Flask or Django", &["python", "web"]);
        add_bookmark(&service, "https://b4.com", "No match", "Unrelated", &["random"]);

        // We want: (All tags => [rust]) OR (Any tags => [web, game]) => that means rust + (web or game).
        // Then exclude ANY => [python], so nothing that has python
        // Then do text search => 'dev'
        // Then limit = 1
        // This is a contrived but thorough example.

        // We'll build the request
        let request = BookmarkSearchRequest {
            query: Some("dev".into()),      // text must contain 'dev'
            all_tags: Some(vec!["rust".into()]), // must have rust
            any_tags: Some(vec!["web".into(), "game".into()]), // must have either web or game
            exclude_any_tags: Some(vec!["python".into()]), // must NOT have python
            limit: Some(1),     // show only the first
            offset: None,
            ..Default::default()
        };

        let resp = service.search_bookmarks(request).unwrap();
        // Bookmarks:
        // b1 => title='Rust Web', desc='Rust+Web dev', tags={rust, web} => has 'dev' text => OK,
        //      also has rust => OK, also has web => OK, no python => pass => total match
        // b2 => title='Rust Gaming', desc='Rust+Game dev', tags={rust, game} => also a match
        // b3 => has python => excluded
        // b4 => no rust => excluded
        // So b1, b2 match, but limit=1

        assert_eq!(resp.total_count, 1);
        assert_eq!(resp.bookmarks.len(), 1);
        let first = &resp.bookmarks[0];
        // Depending on creation order, b1 might appear before b2, or vice versa
        // so we won't strictly check the ID, but let's confirm it's one of them:
        assert!(first.url == "https://b1.com" || first.url == "https://b2.com");
    }
}
