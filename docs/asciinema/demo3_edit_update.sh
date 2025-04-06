#doitlive shell: /bin/bash
#doitlive prompt: damoekri
#doitlive speed: 2
#doitlive commentecho: false
#doitlive alias: setup-environment="source $HOME/dev/s/public/bkmr/docs/asciinema/demo-env.sh"
#doitlive env: BKMR_DB_URL=/tmp/bkmr/bkmr.db

#asciinema rec -i 4 -t "bkmr: Edit and Update" bkmr4-edit-update.cast
#doitlive play /Users/Q187392/dev/s/public/bkmr/docs/asciinema/demo3_edit_update.sh

setup-environment
echo "Create pre-filled demo database"
bkmr create-db --pre-fill /tmp/bkmr/bkmr.db

bkmr search 'github'  # search for term 'github'

bkmr update 1 -t xxx  # add tag 'xxx' to bookmark with id 1

# Show the updated bookmark
bkmr search 'github'  # look for tag: xxx

bkmr update 1 -n xxx  # remove tag 'xxx'
bkmr search 'github'  # look for removed tag: xxx

bkmr edit 1  # edit bookmark with id 1
# (This will open an editor - make some changes to title/description)

echo "Delete bookmarks"
bkmr search --limit 2  # delete in interactive mode
