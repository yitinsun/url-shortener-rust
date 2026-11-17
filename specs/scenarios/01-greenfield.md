# Greenfield scenario

**Intent:** build the URL shortener backend from specs. This file is an evaluation index. It does not generate code.

**Decomposition**

1. Requirements → `specs/greenfield/PRD.md` and `specs/contracts/openapi.yaml` (Gate 1: named APIs, Rust stable edition 2024, MySQL 8, Base62).
2. Architecture → `SYSTEM_DESIGN.md`, `TECH_STACK.md` (Gate 2: a human line `[APPROVED: …]` in `GATE2_APPROVAL.txt`).
3. Implementation in parallel, after approval (Gate 3 sync):
   - **3A** `backend/sql/schema.sql`
   - **3B** `encoding::base62` + unit tests
   - **3C** `backend/Cargo.toml`, SQLx repository, Axum handlers, error response
4. Tests / compliance (Gate 4): `cargo llvm-cov --fail-under-lines 80` from `backend/`, line coverage ≥ 80%, bound SQL, no `#[ignore]`.

**Orchestration path:** Stages 1→2→HITL→3A/3B/3C→4, as in `specs/greenfield/ORCHESTRATION_MANIFEST.md`.

**Validation (when Stage 4 runs):** Base62 tests, service tests, cleanup tests, `tests/url_shortener_api.rs`. Command: `cargo llvm-cov --fail-under-lines 80` on the stable toolchain from `backend/`.

**Outputs present in the workspace:**

- `GATE2_APPROVAL.txt` contains `[APPROVED: yitin]` plus the swagger, legal-path, security, and docs signatures.
- Gate 3 files: `backend/sql/schema.sql`, `backend/Cargo.toml`, `backend/Cargo.lock`, and `backend/src/**` (config, Base62, SQLx repository, Axum handlers, cleanup).
- Gate 4 measurement, `cargo llvm-cov --fail-under-lines 80` from `backend/` at 2026-11-17T13:43:24Z: line coverage **91.21%** (1286 of 1410). 33 library tests and 8 integration tests passed, with zero ignored. `SDLC_METRICS.json` records `line_coverage_ratio` `0.9121`.
- `SDLC_AUDIT_LOG.json` and `SDLC_METRICS.json` exist. `stage1_epoch_ms`, `stage7_epoch_ms`, and `end_to_end_latency_ms` are null. Stage 7 is open.
- Stage 6 artifacts are absent: no `backend/Dockerfile`, no root `docker-compose.yml`, and no image. `docker compose up --build` on 2026-11-17T13:47:33Z exited 1 with `no configuration file provided: not found`.

Policy: `specs/greenfield/ORCHESTRATION_MANIFEST.md`.
