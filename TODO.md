# LLM todo

2-stage search: use rsnip algo
autocompletion like rsnip for snippet titles and types

test coverage and test cases (integration)

json option with embeddings/content_hash

dirs configuration in toml

From trait for errors, simplify/optimize error handling

embedding plugin

docu fzf actions

## Features
- markdown text with ankiview visualization
- rich text with ankiview visualization
- run payload as script (which shell environment?)

### Proposals
- new field 'action': instead of prefix "shell" for action selection
- SOPS integration



## BUGs
sqlite bug: jupy*

section parsing in edit mode: table like ------------

tests fail when ~/.config/bkmr exists and has got wrong config (config tests)



## Gotcha
bkmr search grad*  # expands to gradlew if present

## Marketing
Create technical blog article of how to adreess knowedge management for a software deloper.
Define the general problem:
Store knowledge and make it findable without breaking flow
Make it searchable and findable (FTS, tags, classifications, etc)
Make it actionable (open, execute, copy to clipbard, etc)

bookmarks are a prime manifestation of knowledge
snippets are a prime manifestaion of knowledge
howtos are a prime manifestaion of knowledge
etc...

Explain why this leads to conclusion to combine https://github.com/sysid/rsnip and https://github.com/sysid/bkmr into on
generalized application.

I realized that these two applications actually provide synergies when being combined. Detail them (e.g. user interface,
bigger context, one database, etc...)

So the new bkmr addresses this generalized problem (name it again).

Now go into details of how this problem is addressed and make the benefits of bkmr for the workflow of a developer
obvious. Be specific and clear. Make bkmr irrefutable to use based on it value.


## Advanced
doing column specific prefix search: find out schema: bkrm info --schema, b 'metadata:jq*' (metadata holds the title)
