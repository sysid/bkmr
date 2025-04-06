#doitlive shell: /bin/bash
#doitlive prompt: damoekri
#doitlive speed: 2
#doitlive commentecho: false
#doitlive alias: setup-environment="source $HOME/dev/s/public/bkmr/docs/asciinema/demo-env.sh"

# Source environment and ensure clean state: sss
#asciinema rec -t "bkmr: Getting Started" bkmr_getting_started.cast
#doitlive play /Users/Q187392/dev/s/public/bkmr/docs/asciinema/demo2_search_filter.sh
#asciinema play -i 4 --speed 2 bkmr_getting_started.cast

# Setup environment
setup-environment
bkmr create-db --pre-fill /tmp/bkmr/bkmr.db

echo "Search"
bkmr search -N _snip_,_imported_

bkmr search --fzf
bkmr search --fzf --fzf-style enhanced
echo "Actions: Enter: open, CTRL-E: edit, CTRL-D: delete, CTRL-Y: yank"


echo "Search with tags, execute the command with ENTER"
bkmr search -t shell,rust
