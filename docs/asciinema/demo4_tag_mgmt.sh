# Source environment
source $HOME/dev/s/public/b2/docs/asciinema/demo-env.sh

# Ensure we have bookmarks with a good tag structure
bkmr add https://reactjs.org "React" -d "A JavaScript library for building user interfaces" -t javascript,frontend,framework,react
bkmr add https://vuejs.org "Vue.js" -d "The Progressive JavaScript Framework" -t javascript,frontend,framework,vue
bkmr add https://angular.io "Angular" -d "Platform for building mobile and desktop web applications" -t javascript,frontend,framework,angular
bkmr add https://docs.python.org "Python Documentation" -d "Official Python documentation" -t python,programming,documentation
bkmr add https://docs.ruby-lang.org "Ruby Documentation" -d "Ruby programming language documentation" -t ruby,programming,documentation
bkmr add https://dart.dev/guides "Dart Documentation" -d "Guides for the Dart language" -t dart,programming,documentation


asciinema rec -t "bkmr: Tag Management" bkmr_tags.cast

bkmr tags

bkmr tags programming

# Add bookmarks with hierarchical tags
bkmr add https://docs.python.org tags=programming,python,documentation
bkmr add https://www.typescriptlang.org tags=programming,typescript,documentation

# Show all documentation resources
bkmr search -t documentation

# Filter by programming language
bkmr search -t documentation,python