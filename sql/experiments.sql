SELECT *
FROM bookmarks
WHERE tags NOT LIKE '%_shell_%'
  AND tags NOT LIKE '%_snip_%'
and embeddable
;
