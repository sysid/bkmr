-- This works because SQLite treats boolean expressions as 0/1 integers, so summing them counts matching system tags
-- instr() matches literally; LIKE would treat '_' as a wildcard (',_md_,' matched ',mmdc,')
SELECT id, metadata, tags
FROM bookmarks
WHERE (
    (instr(tags, ',_snip_,') > 0) +
    (instr(tags, ',_shell_,') > 0) +
    (instr(tags, ',_md_,') > 0) +
    (instr(tags, ',_env_,') > 0) +
    (instr(tags, ',_imported_,') > 0) +
    (instr(tags, ',_mem_,') > 0)
) > 1;
