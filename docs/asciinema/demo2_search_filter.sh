#doitlive shell: /bin/bash
#doitlive prompt: damoekri
#doitlive speed: 2
#doitlive commentecho: false
#doitlive alias: setup-environment="source $HOME/dev/s/public/bkmr/docs/asciinema/demo-env.sh"
#doitlive env: BKMR_DB_URL=/tmp/bkmr/bkmr.db

# Source environment and ensure clean state: sss
#asciinema rec -t "bkmr: Search and Filter" bkmr4-search-filter.cast
#doitlive play /Users/Q187392/dev/s/public/bkmr/docs/asciinema/demo2_search_filter.sh
#asciinema play -i 4 --speed 2 bkmr4-search-filter.cast

# Setup environment, used /tmp/bkmr/bkmr.db
setup-environment
echo "Create pre-filled demo database"
bkmr create-db --pre-fill /tmp/bkmr/bkmr.db
clear

echo "Search all except entries with tags _snip_, _imported_"
bkmr search -N _snip_,_imported_
clear

bkmr search --fzf  # use fuzzy finding
clear

# run 4: hello world
bkmr search --fzf --fzf-style enhanced
echo "FZF actions: Enter: open, CTRL-E: edit, CTRL-D: delete, CTRL-Y: yank"
clear

# edit 4 with CTRL-E
bkmr search --fzf --fzf-style enhanced
clear

echo "Search with tags filter, execute the command with 1 ENTER"
bkmr search -t shell 'hello world'
