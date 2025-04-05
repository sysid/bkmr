# Source environment
source $HOME/dev/s/public/b2/docs/asciinema/demo-env.sh

# Create a sample JSON file for import demonstration
cat > ~/bkmr-demos/import_bookmarks.json << 'EOF'
[
  {
    "url": "https://www.mozilla.org",
    "title": "Mozilla",
    "description": "Mozilla Foundation website",
    "tags": ["browser", "opensource", "firefox"]
  },
  {
    "url": "https://kubernetes.io",
    "title": "Kubernetes",
    "description": "Container orchestration platform",
    "tags": ["cloud", "container", "devops"]
  },
  {
    "url": "https://www.docker.com",
    "title": "Docker",
    "description": "Containerization platform",
    "tags": ["container", "devops", "development"]
  },
  {
    "url": "https://www.terraform.io",
    "title": "Terraform",
    "description": "Infrastructure as code software tool",
    "tags": ["infrastructure", "devops", "cloud"]
  }
]
EOF

# Ensure we have different tags in existing bookmarks to show contrast
bkmr add https://nodejs.org "Node.js" -d "JavaScript runtime built on Chrome's V8 JavaScript engine" -t javascript,backend,runtime


asciinema rec -t "bkmr: Import & Export" bkmr_import_export.cast

# Create a JSON export of all bookmarks
bkmr search --json > bookmarks.json

# Show the exported file
cat bookmarks.json | head -20

bkmr load-json import_example.json

# Verify the imported bookmarks
bkmr search mozilla
bkmr search kubernetes