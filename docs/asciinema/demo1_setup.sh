#doitlive shell: /bin/bash
#doitlive prompt: default
#doitlive speed: 2
#doitlive env: DOCS_URL=https://doitlive.readthedocs.io
#doitlive commentecho: false

# Source environment and ensure clean state: sss
#asciinema rec -t "bkmr: Getting Started" bkmr_getting_started.cast
#doitlive play /Users/Q187392/dev/s/public/bkmr/docs/asciinema/demo1_setup.sh
#asciinema play -i 4 --speed 2 bkmr_getting_started.cast

echo "Create configuration"
mkdir -p /tmp/bkmr
bkmr --generate-config > /tmp/bkmr/config.toml
tree /tmp/bkmr

echo "Initialize database."
bkmr create-db /tmp/bkmr/bkmr.db
tree /tmp/bkmr

echo "You can set it also via environment variable"
export BKMR_DB_URL=/tmp/bkmr/demo.db

echo "Now add some data..."
bkmr add https://rust-lang.org programming,rust,language
bkmr add https://github.com programming,git,collaboration
bkmr add https://news.ycombinator.com news,tech

echo "List full database content."
bkmr search
echo "URL metadata has been fetched automatically. Nice!"

echo "Show info about bkmr and its configuration"
bkmr info
