# Contributing to Solana Indexer

Thank you for your interest in contributing to the Solana Indexer! We welcome contributions from the community to help make this the best developer-centric SDK for Solana blockchain data indexing.

## Development Workflow

To ensure a smooth and stable development process, we follow a specific workflow. Please adhere to these guidelines when contributing.

### The `development` Branch

ALL development happens on the `development` branch. This is our integration branch where new features and fixes are merged and tested before being released.

### Workflow Steps

1.  **Fork and Clone**: Fork the repository and clone it to your local machine.
2.  **Checkout Development**: Explicitly checkout `development` to ensure you are on the latest development branch.
    ```bash
    git checkout development
    git pull origin development
    ```
3.  **Create a Feature Branch**: Create a new branch *from* `development` for your specific task (feature, bugfix, etc.).
    *   Use a descriptive name, e.g., `feature/add-new-decoder`, `fix/rpc-timeout`.
    ```bash
    git checkout -b feature/my-cool-feature development
    ```
4.  **Implement Changes**: Write your code, add tests, and ensure everything works locally.
    *   Run tests: `cargo test`
    *   Run benchmarks (if applicable): `cargo bench`
    *   Linting: `cargo clippy`
    *   Formatting: `cargo fmt`
5.  **Commit and Push**: Commit your changes and push your branch to your fork.
6.  **Open a Pull Request**: Open a Pull Request (PR) targeting the **`development`** branch.
    *   **Do NOT target `main` or `master` directly.**
    *   Describe your changes clearly in the PR description.
    *   Link to any relevant issues.

### Merging Process

*   Maintainers will review your PR.
*   Once approved and CI checks pass, your changes will be merged into `development`.
*   **Testing & Benchmarking**: The maintainers will perform additional testing and benchmarking on the `development` branch.
*   **Release**: Once the `development` branch is stable and verified, it will be merged into the main release branch by the maintainers.

## Code Style

*   We use standard Rust coding conventions.
*   Please run `cargo fmt` before submitting your code.
*   Ensure no warnings are present by running `cargo clippy`.

## Reporting Issues

*   If you find a bug or have a feature request, please open an issue on GitHub.
*   Provide as much detail as possible to help us understand and resolve the issue.

Thank you for contributing!
