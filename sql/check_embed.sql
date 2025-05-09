select * from bookmarks
WHERE embeddable = 0 and not content_hash IS NULL;

UPDATE bookmarks
SET content_hash = NULL
WHERE embeddable = 0 OR embedding IS NULL;

UPDATE bookmarks
SET embedding = NULL
WHERE embeddable = 0;
