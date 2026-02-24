# Xenobot Legal-Safe Scope (MacBook arm64 / Apple Silicon)

Last updated: 2026-02-23

## 1. Mandatory legal-safe baseline

Xenobot must not implement or encourage any of the following:

- process memory key extraction
- encryption bypass / anti-protection bypass
- unauthorized database decryption workflows
- SIP-disabling dependent attack paths

Xenobot must implement only user-authorized, local-first, transparent data handling:

- user-provided export files
- official or documented data export interfaces
- OS-provided secure key storage usage (for Xenobot-owned secrets only)
- explicit consent flows and local processing

## 2. Task mapping from original roadmap

Original item `3.1` in the user roadmap mentions process-memory key extraction.
In legal-safe mode, Xenobot replaces it with:

- `3.1-LS1` user-authorized export ingestion connectors
- `3.1-LS2` secure local secret storage via OS keychain APIs
- `3.1-LS3` data source validation + provenance metadata
- `3.1-LS4` no-memory-scraping enforcement checks in CI review

Original item `3.2` mentions decrypting third-party encrypted DB variants.
In legal-safe mode, Xenobot replaces it with:

- `3.2-LS1` import parsers for already-exported plaintext/semi-structured data
- `3.2-LS2` pluggable parser adapters for platform-native export formats
- `3.2-LS3` integrity checks, dedup, and incremental import

## 3. Apple Silicon guidance (M-series / arm64)

For macOS arm64 support, Xenobot should prefer:

- Metal/MPS acceleration only for ML/embedding/ranking compute
- keychain-backed secret handling for Xenobot API keys
- no attempts to access other process memory

## 4. Engineering guardrails

- all risky requests are reviewed against this file before implementation
- if a requested feature conflicts with this policy, implement legal-safe alternatives
- document replacements in commit notes and release notes

