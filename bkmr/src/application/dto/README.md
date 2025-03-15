1. Clean Separation
The DTOs provide a clear boundary between:

External interfaces (CLI, API, UI)
Application services
Domain model

2. Type Safety

Each operation has its own request/response types
Mapping functions enforce proper transformations
Type errors are caught at compile time

3. Documentation

DTOs serve as self-documenting interface contracts
Serialization attributes (Serialize/Deserialize) make API compatibility clear
Request/response structure clearly shows what data is needed for each operation

4. Flexibility

Mapping functions isolate changes between layers
DTOs can evolve independently of domain model
Different representations (list vs. detail) are explicit

5. Modularity

Organized by domain concept (bookmark vs. tag)
Can be extended with new DTOs without affecting existing code
Re-exports make common DTOs easy to access