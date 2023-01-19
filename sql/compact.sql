BEGIN TRANSACTION;
DELETE
FROM bookmarks
WHERE id = :deleted_id;
UPDATE bookmarks
SET id = id - 1
WHERE id > :deleted_id;
COMMIT;

BEGIN TRANSACTION;
DELETE
FROM bookmarks
WHERE id = :deleted_id
returning *;
UPDATE bookmarks
SET id = id - 1
WHERE id > :deleted_id;
COMMIT;

-- Variant 2
BEGIN TRANSACTION;

-- create a temporary table with all rows except the one to be deleted
CREATE TEMP TABLE temp AS
SELECT *
FROM book
WHERE id != {id_to_delete};

-- delete the original table
DROP TABLE table_name;

-- rename the temporary table to the original table
ALTER TABLE temp
    RENAME TO table_name;

COMMIT;
