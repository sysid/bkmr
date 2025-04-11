create table bookmarks
(
    id             INTEGER not null primary key,
    URL            VARCHAR not null unique,
    metadata       VARCHAR not null default '',
    tags           VARCHAR not null default '',
    desc           VARCHAR not null default '',
    flags          INTEGER not null default 0,
    last_update_ts DATETIME not null default CURRENT_TIMESTAMP
);

CREATE TRIGGER [UpdateLastTime]
    AFTER UPDATE
    ON bookmarks
    FOR EACH ROW
    WHEN NEW.last_update_ts <= OLD.last_update_ts
BEGIN
    update bookmarks set last_update_ts=CURRENT_TIMESTAMP where id = OLD.id;
END;

create virtual table bookmarks_fts using fts5
(
    id,
    URL,
    metadata,
    tags,
    "desc",
    flags UNINDEXED,
    last_update_ts UNINDEXED,
    content= 'bookmarks',
    content_rowid= 'id',
    tokenize= "porter unicode61"
);

CREATE TRIGGER bookmarks_ad
    AFTER DELETE
    ON bookmarks
BEGIN
    INSERT INTO bookmarks_fts (bookmarks_fts, rowid, URL, metadata, tags, "desc")
    VALUES ('delete', old.id, old.URL, old.metadata, old.tags, old.desc);
END;

CREATE TRIGGER bookmarks_ai
    AFTER INSERT
    ON bookmarks
BEGIN
    INSERT INTO bookmarks_fts (rowid, URL, metadata, tags, "desc")
    VALUES (new.id, new.URL, new.metadata, new.tags, new.desc);
END;

CREATE TRIGGER bookmarks_au
    AFTER UPDATE
    ON bookmarks
BEGIN
    INSERT INTO bookmarks_fts (bookmarks_fts, rowid, URL, metadata, tags, "desc")
    VALUES ('delete', old.id, old.URL, old.metadata, old.tags, old.desc);
    INSERT INTO bookmarks_fts (rowid, URL, metadata, tags, "desc")
    VALUES (new.id, new.URL, new.metadata, new.tags, new.desc);
END;

/*
create table bookmarks_fts_config
(
    k not null primary key,
    v
)
    without rowid;

create table bookmarks_fts_data
(
    id INTEGER primary key,
    block BLOB
);

create table bookmarks_fts_docsize
(
    id INTEGER primary key,
    sz BLOB
);

create table bookmarks_fts_idx
(
    segid not null,
    term  not null,
    pgno,
    primary key (segid, term)
)
    without rowid;

*/
insert into main.bookmarks (URL, metadata, tags, "desc", flags)
values
   ('https://www.google.com', 'Google', ',ccc,yyy,', 'Example Entry', 0),
   ('http://xxxxx/yyyyy', 'TEST: entry for bookmark xxxx', ',ccc,xxx,yyy,', 'nice description b', 0),
   ('http://aaaaa/bbbbb', 'TEST: entry for bookmark bbbb', ',aaa,bbb,', 'nice description a', 0),
   ('http://asdf/asdf', 'bla blub', ',aaa,bbb,', 'nice description a2', 0),
   ('http://asdf2/asdf2', 'bla blub2', ',aaa,bbb,ccc,', 'nice description a3', 0),
   ('http://11111/11111', 'bla blub3', ',aaa,bbb,ccc,', 'nice description a4', 0),
   ('http://none/none', '', ',,', '', 0),
   ('/Users/Q187392', 'home', ',,', '', 0),
   ('$HOME/dev', 'dev', ',,', '', 0),
   ('$HOME/dev/s/public/bkmr/bkmr/tests/resources/bkmr.pptx', 'pptx', ',,', '', 0),
   ('https://example.com/{{ env_USER }}/dashboard', 'Checking jinja', ',,', '', 0),
   ('text with environment varialbe default: {{ env("MY_VAR", "ENV_FALLBACK_VALUE") }}/dashboard', 'env', ',_snip_,', '', 0),
   ('bkmr/tests/resources/sample_docu.md', 'markdown file', ',_md_,', '', 0),
   ('shell::vim +/"## SqlAlchemy" $HOME/dev/s/public/bkmr/bkmr/tests/resources/sample_docu.md', 'shell open vim', ',,', '', 0)
   ;
