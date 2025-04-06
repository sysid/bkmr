echo "-M- bkmr demo environment"
rm -fr /tmp/bkmr
unset BKMR_DB_URL
unset BKMR_FZF_OPTS

#export BKMR_DB_URL=~/xxx/bkmr-demos/demo.db
export EDITOR=vim

export COLUMNS=100
export LINES=30

mkdir -p /tmp/bkmr > /dev/null 2>&1
#!/bin/bash
NAME="Alice"

cat <<_EOF_ > /tmp/bkmr/config.toml
db_url = "/tmp/bkmr/bkmr.db"

[fzf_opts]
height = "50%"
reverse = false
show_tags = false
no_url = false
_EOF_

export BKMR_DB_URL=/tmp/bkmr/bkmr.db

touch ~/xxx/rust-files1.rs
touch ~/xxx/rust-files2.rs

echo "-M- BKMR_DB_URL: $BKMR_DB_URL"
#tree /tmp/bkmr
