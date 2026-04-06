-- name: get_all
-- select count(*)
select *
from bookmarks
where embeddable = 0;

select *
from bookmarks_fts
where bookmarks_fts match 'xxx' -- :fts_query
order by rank;


--- delete embeddings
update bookmarks
set embedding = NULL
where embedding IS NOT NULL;

update bookmarks
set content_hash = NULL
where content_hash IS NOT NULL;

/*
 For tracking the database version, I use the built in user-version variable that sqlite provides
 (sqlite does nothing with this variable, you are free to use it however you please).
 It starts at 0, and you can get/set this variable with the following sqlite statements:
 */
-- name: get_user_version
PRAGMA user_version;

-- PRAGMA user_version = 1;

-- name: get_related_tags
with RECURSIVE split(tags, rest) AS (SELECT '', tags || ','
                                     FROM bookmarks
                                     WHERE tags LIKE '%,ccc,%'
                                     UNION ALL
                                     SELECT substr(rest, 0, instr(rest, ',')),
                                            substr(rest, instr(rest, ',') + 1)
                                     FROM split
                                     WHERE rest <> '')
SELECT distinct tags
FROM split
WHERE tags <> ''
ORDER BY tags;


-- name: get_all_tags
with RECURSIVE split(tags, rest) AS (SELECT '', tags || ','
                                     FROM bookmarks
                                     UNION ALL
                                     SELECT substr(rest, 0, instr(rest, ',')),
                                            substr(rest, instr(rest, ',') + 1)
                                     FROM split
                                     WHERE rest <> '')
SELECT distinct tags
FROM split
WHERE tags <> ''
ORDER BY tags;

 SELECT 1 as diesel_exists
 FROM sqlite_master WHERE type='table' AND name='__diesel_schema_migrations';

-- name: get_embeddable_imported
-- Use instr() for exact token matching; LIKE treats '_' as a wildcard.
select *
from bookmarks
where embeddable = 1
  and instr(tags, ',_imported_,') > 0;

-- name: delete_embeddable_imported
delete from bookmarks
where embeddable = 1
  and instr(tags, ',_imported_,') > 0;

-- name: mark_snip_non_embeddable
-- UPDATE bookmarks SET embeddable = 0 WHERE tags LIKE '%_snip_%';
UPDATE bookmarks SET embeddable = 1 WHERE tags LIKE '%_mem_%';

select *
from bookmarks
where tags LIKE '%_shell_%'
and embeddable = 1;


-- This query is for finding bookmarks that have tags but none of the tags are "mem", "snip", or "shell".
SELECT count(*)
FROM bookmarks WHERE tags NOT GLOB '*,_*_,*';

-- clear all embeddings
-- After this, bkmr backfill will regenerate embeddings for all embeddable = 1 bookmarks from scratch.
-- Note: Don't try to DROP and recreate vec_bookmarks — the virtual table and its shadow tables (vec_bookmarks_chunks, vec_bookmarks_rowids, etc.) are managed by sqlite-vec internally. A simple DELETE is the clean way.
DELETE FROM vec_bookmarks;
UPDATE bookmarks SET content_hash = NULL;


