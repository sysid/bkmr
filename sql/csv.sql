.mode csv
.output csv.sql
select id, url, metadata, tags, desc, flags, last_update_ts from bookmarks;
.output stdout
