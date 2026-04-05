# Source environment
source $HOME/dev/s/public/b2/docs/asciinema/demo-env.sh

# No API key needed — semantic search is fully local!

# Add bookmarks with rich descriptions for semantic search
bkmr add https://arxiv.org/abs/1706.03762 ai,nlp,research,paper --title "Attention Is All You Need" -d "The original paper that introduced the transformer architecture, which has revolutionized natural language processing and many other areas of machine learning"
bkmr add https://jalammar.github.io/illustrated-transformer/ ai,nlp,tutorial,visualization --title "The Illustrated Transformer" -d "A visual and intuitive explanation of how transformers work in natural language processing"
bkmr add https://arxiv.org/abs/1810.04805 ai,nlp,bert,research --title "BERT Paper" -d "Bidirectional Encoder Representations from Transformers, a groundbreaking language representation model"
bkmr add https://openai.com/research/chatgpt ai,chatgpt,conversational,llm --title "ChatGPT Blog Post" -d "OpenAI's article introducing ChatGPT, a conversational AI model trained to be helpful, harmless, and honest"

# Mark bookmarks as embeddable and backfill
bkmr backfill

asciinema rec -t "bkmr: Semantic Search" bkmr_semantic_search.cast

echo "Let's try semantic search capabilities where we search by meaning rather than keywords"

bkmr sem-search "How do transformers work in deep learning?"
