---
name: url-shortener-sdlc
description: >-
  Runs the gated spec-first Rust SDLC for this repo (PRD, design, stack,
  tests, Gate 2 HITL, llvm-cov, audit log). Use when implementing the URL
  shortener backend from specs/greenfield, or when the user mentions AGENTS.md,
  Gate 2, or the backend SDLC.
disable-model-invocation: true
---

# URL shortener backend SDLC

Cursor loads this file when the skill is attached. Product behavior lives in `specs/greenfield/`. Do not invent APIs, Cargo dependencies, or coverage.

## Instructions

1. Read `AGENTS.md`.
2. Read `specs/greenfield/INSTRUCTIONS.md` and `specs/greenfield/ORCHESTRATION_MANIFEST.md`.
3. Read `PRD.md`, `SYSTEM_DESIGN.md`, `TECH_STACK.md`, and `TESTING_PLAN.md` in `specs/greenfield/` for their roles only.
4. Keep `specs/contracts/openapi.yaml` aligned with `PRD.md`.
5. Honor Gate 2: no `backend/Cargo.toml`, `backend/Cargo.lock`, `backend/src/**`, or `backend/sql/**` until `GATE2_APPROVAL.txt` contains `[APPROVED: …]`.
6. After approval, run 3A schema, 3B Base62, and 3C the Axum crate together, then `cargo llvm-cov --fail-under-lines 80` from `backend/`.
7. Brownfield playbooks in `specs/brownfield/` start only after Gate 4. `specs/scenarios/` records evaluation; it does not generate code.
8. Do not create `frontend/src/**` in this skill. The React specs live in `specs/frontend/` and are a separate pass.
