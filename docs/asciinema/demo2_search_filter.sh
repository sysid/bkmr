#doitlive shell: /bin/bash
#doitlive prompt: damoekri
#doitlive speed: 2
#doitlive commentecho: false
#doitlive alias: setup-environment="source $HOME/dev/s/public/bkmr/docs/asciinema/demo-env.sh"
#doitlive env: BKMR_DB_URL=/tmp/bkmr/bkmr.db

# Source environment and ensure clean state: sss
#asciinema rec -t "bkmr: Getting Started" demo2.cast
#doitlive play /Users/Q187392/dev/s/public/bkmr/docs/asciinema/demo2_search_filter.sh
#asciinema play -i 4 --speed 2 bkmr_getting_started.cast

# Setup environment, used /tmp/bkmr/bkmr.db
setup-environment
bkmr create-db --pre-fill /tmp/bkmr/bkmr.db

bkmr search -N _snip_,_imported_

bkmr search --fzf
# run 4: hello world
bkmr search --fzf --fzf-style enhanced
echo "FZF actions: Enter: open, CTRL-E: edit, CTRL-D: delete, CTRL-Y: yank"

# edit 4 with CTRL-E
bkmr search --fzf --fzf-style enhanced


echo "Search with tags, execute the command with 1 ENTER"
bkmr search -t shell 'hello world'
