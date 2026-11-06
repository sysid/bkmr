# Contributing to bkmr

Bug fixes and improvements are welcome. For a significant change, open an
[issue](https://github.com/sysid/bkmr/issues) first so we can agree on the approach. Please be
respectful to other contributors and users.

## Setup

You need stable Rust. SQLite is bundled.

```bash
git clone https://github.com/<your-account>/bkmr.git   # your fork
cd bkmr
cargo build --manifest-path bkmr/Cargo.toml             # the manifest is in bkmr/, not the root
make test                                               # single-threaded; see TESTING.md
```

## Making a change

1. Branch from `main`, e.g. `feat/<name>` or `fix/<name>`.
2. Write a failing test first, then the code. See [TESTING.md](TESTING.md) for test conventions.
3. Before opening the pull request, run:

   ```bash
   make format
   make lint
   make test
   ```
4. Update the README and [wiki](https://github.com/sysid/bkmr/wiki) if behaviour changes.
5. In the pull request, describe the change and link related issues.

## Where things go

The code uses a layered architecture. Dependencies point inward:

| Directory (`bkmr/src/`) | Holds | May depend on |
|---|---|---|
| `domain` | Entities, business rules, repository traits | nothing |
| `application` | Services and use cases | `domain` |
| `infrastructure` | SQLite, embeddings, clipboard, HTTP | implements `domain` traits |
| `cli`, `lsp` | Command line and editor entry points | `application` services |

The binding rules are under "Architectural Invariants" in [AGENTS.md](AGENTS.md). For errors and
exit codes, see [ERROR_HANDLING.md](ERROR_HANDLING.md). For architecture and build details, see
the wiki's [Development](https://github.com/sysid/bkmr/wiki/Development) page.

## Releases (maintainers)

Versions follow [Semantic Versioning](https://semver.org/). To release, run one of:

```bash
make bump-patch   # or bump-minor, bump-major
```

This runs immediately and is visible to everyone: it bumps `VERSION`, `pyproject.toml`,
`bkmr/Cargo.toml` and `bkmr/Cargo.lock` in one commit, tags it, pushes, and creates a GitHub
release with generated notes. It needs `bump-my-version`, `gh` and `GITHUB_TOKEN`.
