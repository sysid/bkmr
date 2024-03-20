-- sqlite3 /Users/Q187392/xxx/bm.db <csv.sql
.mode csv
.output out.csv
select url, metadata, tags, desc, flags, last_update_ts from bookmarks;
.output stdout
