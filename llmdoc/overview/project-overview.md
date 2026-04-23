# Project Overview

## Identity

Albert is a local-first, AI-ready API mock desktop application. It is intended to reduce mock authoring effort by converting API descriptions into normalized, reusable mock assets.

## Boundaries

Belongs here:

- desktop control surface
- canonical API modeling
- parser, storage, gateway, and provider boundaries
- future AI-assisted mock generation workflow

Does not belong here:

- backend business logic of user projects
- cloud deployment concerns
- test-runner specific mocking integrations

## Major Areas

- `apps/desktop`: operator UI
- `crates/albert-core`: shared domain contracts
- `crates/albert-parser`: ingestion
- `crates/albert-storage`: persistence
- `crates/albert-gateway`: local runtime boundary
- `crates/albert-openai`: provider adapter

