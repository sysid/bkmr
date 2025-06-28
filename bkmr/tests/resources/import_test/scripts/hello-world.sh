# name: hello-world
# tags: asdf
# type: _shell_
#!/usr/bin/env bash
set -Eeuo pipefail +x

source ~/dev/binx/profile/sane_fn.sh

Green "-M- Hello World from bash_stubs: $(pwd)"
echo "Args: $@"
exit 0
