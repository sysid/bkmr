WITH Sequenced AS (
  SELECT
    id,
    LEAD(id) OVER (ORDER BY id) AS next_id  -- calculates the id of the next row for each row, based on the order of id.
  FROM
    bookmarks
)
SELECT
  id + 1 AS gap_start,
  next_id - 1 AS gap_end
FROM
  Sequenced
WHERE
  next_id - id > 1;
