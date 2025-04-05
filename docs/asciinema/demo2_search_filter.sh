# Source environment
source $HOME/dev/s/public/b2/docs/asciinema/demo-env.sh

# Ensure we have sufficient bookmarks to demonstrate search
bkmr add https://rust-lang.org "Rust Programming Language" -d "A language empowering everyone to build reliable and efficient software" -t programming,rust,language
bkmr add https://github.com "GitHub" -d "Where the world builds software" -t programming,git,collaboration,platform
bkmr add https://news.ycombinator.com "Hacker News" -d "A social news website focusing on computer science and entrepreneurship" -t news,tech,community
bkmr add https://dev.to "DEV Community" -d "A constructive and inclusive social network for software developers" -t programming,community,blog
bkmr add https://stackoverflow.com "Stack Overflow" -d "Public platform for developers to learn and share programming knowledge" -t programming,qa,community
bkmr add https://python.org "Python" -d "A programming language that lets you work quickly and integrate systems effectively" -t programming,python,language


asciinema rec -t "bkmr: Search & Filtering" bkmr_search.cast

# Search for bookmarks with "programming" tag
bkmr search -t programming

# Search for bookmarks with both "programming" and "rust" tags
bkmr search -t programming,rust

# Search for bookmarks with any of "news" or "tech" tags
bkmr search -n news,tech

# Full text search
bkmr search github

# Combine text search with tag filtering
bkmr search rust -t programming

