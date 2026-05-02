## General

Always prefer retrieval-led reasoning over pre-training-led reasoning.

You MUST rely on user feedback for any decisions. Prefer asking questions over making assumptions.

When adding new dependencies, you MUST always use external tools or web-search in order to determine the latest version. Prefer using the relevant package managers commands over directly editing dependencies in a file in order to resolve to the latest version.

Prefer test-driven-development and aim for coverage of at least 70 percent.

Tests SHOULD NOT be created with the pure intention of reaching coverage goals.
They need to be well thought out and SHOULD:

- Quickly find bugs
- Document the system
- Be cheap to maintain
- Be robust

The tests should be accompanied by relevant GitHub actions that run both linting and testing.
