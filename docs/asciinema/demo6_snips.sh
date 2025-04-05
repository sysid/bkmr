# Source environment
source $HOME/dev/s/public/b2/docs/asciinema/demo-env.sh

# Create a couple of example snippets beforehand to show retrieval
cat > ~/bkmr-demos/rust-error.txt << 'EOF'
// Rust Error Handling Example
fn read_file() -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open("file.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}
EOF

cat > ~/bkmr-demos/python-list.txt << 'EOF'
# Python List Comprehension Examples
squares = [x**2 for x in range(10)]
evens = [x for x in range(10) if x % 2 == 0]
matrix_flatten = [item for row in matrix for item in row]
EOF

# Pre-add one snippet for demonstration
bkmr add --type snip --title "Python List Comprehensions" --tags python,snippet,list "$(cat ~/bkmr-demos/python-list.txt)"


asciinema rec -t "bkmr: Advanced Features" bkmr_advanced.cast

bkmr add --type snip --edit
# (In the editor, add a title like "Rust Error Handling")
# (In the URL section, add a code snippet:)

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::open("file.txt")?;
    Ok(())
}

bkmr search snip
bkmr show 7  # Replace with the actual ID

bkmr add --edit
# (Show the template format and how it guides data entry)