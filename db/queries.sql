-- name: get_all
select *
from bookmarks;

select *
from bookmarks_fts
where bookmarks_fts match 'xxx' -- :fts_query
order by rank;


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