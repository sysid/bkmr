# Source environment
source $HOME/dev/s/public/b2/docs/asciinema/demo-env.sh

# Set up OpenAI API key (if you're demonstrating this feature)
export OPENAI_API_KEY="your-api-key-here"  # Replace with actual key or use a placeholder

# Add bookmarks with rich descriptions for semantic search
bkmr add --openai https://arxiv.org/abs/1706.03762 "Attention Is All You Need" -d "The original paper that introduced the transformer architecture, which has revolutionized natural language processing and many other areas of machine learning" -t ai,nlp,research,paper
bkmr add --openai https://jalammar.github.io/illustrated-transformer/ "The Illustrated Transformer" -d "A visual and intuitive explanation of how transformers work in natural language processing" -t ai,nlp,tutorial,visualization
bkmr add --openai https://arxiv.org/abs/1810.04805 "BERT Paper" -d "Bidirectional Encoder Representations from Transformers, a groundbreaking language representation model" -t ai,nlp,bert,research
bkmr add --openai https://openai.com/research/chatgpt "ChatGPT Blog Post" -d "OpenAI's article introducing ChatGPT, a conversational AI model trained to be helpful, harmless, and honest" -t ai,chatgpt,conversational,llm

# Backfill embeddings
bkmr backfill --openai

asciinema rec -t "bkmr: Semantic Search" bkmr_semantic_search.cast

echo "Let's try semantic search capabilities where we search by meaning rather than keywords"

bkmr semsearch "How do transformers work in deep learning?"

