# Contributing to BKMR

Thank you for considering contributing to BKMR! This document outlines the process for contributing to the project and provides guidelines to ensure consistent code quality.

## Code of Conduct

Please be respectful to all contributors and users. We aim to foster an inclusive and welcoming community.

## Getting Started

### Prerequisites

- Rust (stable channel)
- Cargo
- SQLite development libraries

### Setting Up Development Environment

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/bkmr.git
   cd bkmr
   ```

2. Build the project:
   ```bash
   cargo build
   ```

3. Run tests:
   ```bash
   cargo test
   ```

## Development Workflow

1. Create a new branch for your feature or fix:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. Make your changes following the code style guidelines below.

3. Write tests for your changes.

4. Run tests locally:
   ```bash
   cargo test
   ```

5. Submit a pull request.

## Code Style Guidelines

### Architecture

The project follows clean architecture principles with an onion model:

- `domain`: Core business logic and entities
- `application`: Use cases and services
- `infrastructure`: External systems integration
- `cli`: Command-line interface

### Naming Conventions

- Use descriptive names that convey intent
- For test functions, follow the pattern: `given_X_when_Y_then_Z()`
- Use snake_case for functions, variables, and file names
- Use PascalCase for structs, enums, and traits

### Error Handling

- Use `thiserror` for error definitions
- Provide meaningful error messages

### Testing

- Write unit tests for all public functions
- Follow the Arrange/Act/Assert pattern

Example test structure:
```rust
#[test]
fn given_valid_input_when_parsing_then_returns_expected_result() {
    // Arrange
    let input = "valid input";
    
    // Act
    let result = parse_input(input);
    
    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected_value);
}
```

### Documentation

- Document all public APIs
- Include examples in documentation where appropriate
- Keep README up to date

## Pull Request Process

1. Ensure all tests pass
2. Update documentation if necessary
3. Add a clear description of the changes
4. Link related issues

## Release Process

1. Version numbers follow [Semantic Versioning](https://semver.org/)
2. Releases are created from the `main` branch
3. Each release includes a changelog entry

## Finding Tasks to Work On

- Check the issues tab for tasks labeled "good first issue"
- Feel free to ask for clarification or help on any issue

Thank you for contributing to BKMR!
