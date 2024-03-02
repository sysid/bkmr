import os

from openai import OpenAI
client = OpenAI(
    # This is the default and can be omitted
    api_key=os.environ.get("OPENAI_API_KEY"),
)

# # Replace "your_api_key" with your actual OpenAI API key
# openai.api_key = os.getenv("OPENAI_API_KEY")
# print(openai.api_key)


def get_embeddings(text: str) -> list:
    response = client.embeddings.create(
        input=text.replace("\n", " "),
        model="text-embedding-ada-002"  # Example model; choose the model as per your requirement
    )
    embeddings = response.data[0].embedding  # This extracts the embedding vector
    return embeddings


# Example usage
text = "Hello, world!"
embeddings = get_embeddings(text)
print(embeddings)
