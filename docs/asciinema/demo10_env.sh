# run in asciinema, 10x zoom
unset FOO
source demo-env.sh

bkmr create-db --pre-fill /tmp/bkmr/bkmr.db  # Create pre-filled demo databas
bkmr search --fzf
clear

# NOT WORKING (panick)
echo "Alias to source output of bkmr into environment"
alias set-env="source <(bkmr search --fzf --fzf-style enhanced -t _env_)"
clear

bkmr add -t env  # add some variables
echo $FOO  # variable not set in environment
set-env
echo $FOO  # variable now set
