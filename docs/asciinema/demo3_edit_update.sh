# Source environment
source $HOME/dev/s/public/b2/docs/asciinema/demo-env.sh

# Ensure we have specific bookmarks to update
bkmr add https://www.postgresql.org "PostgreSQL" -d "The world's most advanced open source database" -t database,opensource
bkmr add https://developer.mozilla.org "MDN Web Docs" -d "Resources for developers, by developers" -t programming,webdev,documentation
bkmr add https://css-tricks.com "CSS-Tricks" -d "Tips, Tricks, and Techniques on using CSS" -t webdev,css,frontend

# Check IDs for reference during demo
bkmr search


asciinema rec -t "bkmr: Managing Bookmarks" bkmr_management.cast

bkmr search

# Add a tag to bookmark 1
bkmr update 1 -t favorites

# Show the updated bookmark
bkmr show 1

# Remove a tag from bookmark 2
bkmr update 2 -n git

# Show the updated bookmark
bkmr show 2

bkmr edit 3
# (This will open an editor - make some changes to title/description)

# Create a temporary bookmark to delete
bkmr add https://example.com tags=temp

# Search for the new bookmark
bkmr search temp

# Delete the bookmark (assuming it's ID 4)
bkmr delete 4

