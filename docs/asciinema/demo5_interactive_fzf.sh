# Source environment
source $HOME/dev/s/public/b2/docs/asciinema/demo-env.sh

# Ensure FZF is installed
which fzf || echo "Please install fzf before recording this demo"

# Add some bookmarks with descriptive titles and varied content to demonstrate fuzzy finding
bkmr add https://tailwindcss.com "Tailwind CSS" -d "A utility-first CSS framework" -t css,frontend,framework
bkmr add https://getbootstrap.com "Bootstrap" -d "The most popular HTML, CSS, and JS library" -t css,frontend,framework
bkmr add https://material-ui.com "Material UI" -d "React components for faster and easier web development" -t react,frontend,components
bkmr add https://bulma.io "Bulma" -d "Free, open source CSS framework" -t css,frontend,framework


asciinema rec -t "bkmr: Interactive Features" bkmr_interactive.cast

bkmr search --fzf
# (Type a few characters to filter)
# (Press ESC to cancel)

bkmr search --fzf
# (Filter down to a bookmark)
# (Press Enter to select)
# (This would typically open a browser, but for demo purposes it will just show the command)

bkmr search
# (Show typing a command like "e 1" to edit bookmark 1)
# (Show typing "d 2" to delete bookmark 2)
# (Show typing "p" to print all IDs)
