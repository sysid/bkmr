-- This works because SQLite treats boolean expressions as 0/1 integers, so summing them counts matching system tags
SELECT id, metadata, tags
FROM bookmarks
WHERE (
    (tags LIKE '%,_snip_,%') +
    (tags LIKE '%,_shell_,%') +
    (tags LIKE '%,_md_,%') +
    (tags LIKE '%,_env_,%') +
    (tags LIKE '%,_imported_,%') +
    (tags LIKE '%,_mem_,%')
) > 1;
