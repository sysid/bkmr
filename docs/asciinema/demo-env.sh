echo "-M- bkmr demo environment"
rm -vfr ~/.config/bkmr
unset BKMR_DB_URL
unset BKMR_FZF_OPTS

#export BKMR_DB_URL=~/xxx/bkmr-demos/demo.db
export EDITOR=vim

export COLUMNS=100
export LINES=30

return  # comment for partial init

mkdir -p ~/.config/bkmr
cp /Users/Q187392/dev/s/public/b2/docs/asciinema/demo1.db ~/.config/bkmr/bkmr.db

echo "-M- BKMR_DB_URL: $BKMR_DB_URL"
tree ~/.config/bkmr
