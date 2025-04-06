#doitlive shell: /bin/bash
#doitlive prompt: damoekri
#doitlive speed: 2
#doitlive commentecho: false
#doitlive alias: setup-environment="source $HOME/dev/s/public/bkmr/docs/asciinema/demo-env.sh"
#doitlive env: BKMR_DB_URL=/tmp/bkmr/bkmr.db

#asciinema rec -i 4 -t "bkmr: Tag Management" bkmr4-tag-mgmt.cast
#doitlive play /Users/Q187392/dev/s/public/bkmr/docs/asciinema/demo4_tag_mgmt.sh

setup-environment
echo "Create pre-filled demo database"
bkmr create-db --pre-fill /tmp/bkmr/bkmr.db

bkmr search  # show the data

echo "Tag management"
bkmr tags  # list all tags and their frequency

bkmr tags _snip_  # list all tags which occur together with tag '_snip_'
