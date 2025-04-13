#doitlive shell: /bin/bash
#doitlive prompt: damoekri
#doitlive speed: 2
#doitlive commentecho: false
#doitlive alias: setup-environment="source $HOME/dev/s/public/bkmr/docs/asciinema/demo-env.sh"
#doitlive env: BKMR_DB_URL=/tmp/bkmr/bkmr.db

# Source environment and ensure clean state: sss
#asciinema rec -i 4 -t "bkmr: Overview" bkmr4-all.cast
#doitlive play /Users/Q187392/dev/s/public/bkmr/docs/asciinema/demo11_all.sh

setup-environment
bkmr create-db --pre-fill /tmp/bkmr/bkmr.db
clear

bkmr info
clear

# show interactive h
bkmr search -N _snip_,_shell_,_md_,_env_,_imported_  # default view, only URLs
clear

bkmr search --help | grep fzf  # look at the CTRL-x commands
clear

bkmr search --fzf  # view with fzf selection
clear
bkmr search --fzf --fzf-style enhanced --tags _snip_  # snippet view
clear

# run 4: hello world ENTER
bkmr search --fzf --fzf-style enhanced --tags _shell_  # shell scripts, run with ENTER
clear

# edit 4 with CTRL-E
bkmr search --fzf --fzf-style enhanced  # edit: CTRL-E
clear

bkmr search -t shell 'hello world'  # uniq hit -> default execution
