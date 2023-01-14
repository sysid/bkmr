select *
from bookmarks
where id is null
   or URL is null
   or metadata is null
   or tags is null
   or desc is null
   or flags is null
   or last_update_ts is null
;

delete from bookmarks
where id = 1607
;
