# Contributing to Isochron

Thank you for your interest in contributing to Isochron! This document provides guidelines for contributing to the project.

## Code of Conduct

Be respectful and constructive in all interactions. We're building something together.

## Getting Started

### Prerequisites

- Rust toolchain (stable)
- Embedded target: `rustup target add thumbv6m-none-eabi`
- For hardware testing: SKR Pico board + debug probe

### Building

```bash
# Build all crates
cargo build

# Build firmware specifically
cd isochron-firmware
cargo build --release
```

### Running Tests

```bash
# Run tests for host-compatible crates
cargo test -p isochron-core
cargo test -p isochron-protocol
```

## How to Contribute

### Reporting Bugs

- Use the bug report issue template
- Include firmware version/commit hash
- Describe expected vs actual behavior
- Include relevant config snippets if applicable

### Suggesting Features

- Use the feature request issue template
- Explain the use case and why it's valuable
- Consider how it fits with the Klipper-inspired config philosophy

### Submitting Changes

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes
4. Run `cargo build` and `cargo test` to verify
5. Run `cargo clippy` and fix any warnings
6. Submit a pull request

## Code Style

### Rust

- Follow standard Rust formatting (`cargo fmt`)
- No clippy warnings (`cargo clippy`)
- Use `defmt` for logging in firmware code
- Prefer `heapless` collections over `alloc` where possible

### Commits

- Use conventional commit format: `type: description`
  - `feat:` - New feature
  - `fix:` - Bug fix
  - `docs:` - Documentation changes
  - `refactor:` - Code restructuring
  - `test:` - Adding tests
  - `chore:` - Maintenance tasks
- Keep commits focused and atomic
- Write clear commit messages explaining *why*, not just *what*

### Documentation

- Document public APIs with doc comments
- Update relevant docs/ files when changing behavior
- Include examples in doc comments where helpful

## Architecture Notes

### Crate Structure

- `isochron-core` - Board-agnostic logic (state machine, scheduler, safety)
- `isochron-drivers` - Hardware driver implementations
- `isochron-hal-rp2040` - RP2040-specific HAL code
- `isochron-protocol` - Display communication protocol
- `isochron-firmware` - Main binary, ties everything together

### Key Principles

- **Config-driven**: Hardware defined in TOML, not hardcoded
- **Safety first**: All operations check safety conditions
- **Portable**: Core logic should work across different MCUs

## Questions?

Open a discussion on GitHub if you're unsure about anything.
