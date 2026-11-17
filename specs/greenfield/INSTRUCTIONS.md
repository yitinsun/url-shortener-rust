# AI Agent Instructions: Autonomous Rust URL Shortener with Lifecycle Governance

You are an advanced, context-aware AI software engineering node operating inside a stateful orchestration graph. You will build, test, and containerize the URL shortener **backend** under strict compliance and observability standards matching Rust stable (edition 2024) and MySQL 8.0.

Governing documents (do not contradict them):

* Product: `specs/greenfield/PRD.md`
* Architecture: `specs/greenfield/SYSTEM_DESIGN.md`
* Toolchain: `specs/greenfield/TECH_STACK.md`
* Tests: `specs/greenfield/TESTING_PLAN.md`
* Lifecycle gates: `specs/greenfield/ORCHESTRATION_MANIFEST.md`
* Halt rules: `AGENTS.md`

Do not generate `frontend/**` from these instructions.

## 1. Observability, Lineage & Decision Logging

Maintain a continuous file named `SDLC_AUDIT_LOG.json` in the **workspace root**. The file is a JSON array. Append one object for every file generation, module modification, test execution, or self-correction run:

```json
{
  "timestamp": "2026-11-16T00:00:00Z",
  "stage": "IMPLEMENTATION",
  "action_taken": "Created encoding/base62.rs to implement SYSTEM_DESIGN.md section 3.",
  "upstream_dependency": "specs/greenfield/SYSTEM_DESIGN.md",
  "upstream_dependency_hash": "<SHA-256 of the governing markdown file>",
  "rationale": "Deterministic ID-to-Base62 encoding avoids hash-collision retries on insert."
}
```

Allowed `stage` values: `REQUIREMENTS`, `ARCHITECTURE`, `IMPLEMENTATION`, `TESTING`, `COMPLIANCE`, `OBSERVABILITY`, `RELEASE`, `REPAIR`, `REPLAN`.

Preserve historical entries. Never overwrite the log on re-plan; only append. Do not write `"n/a"` for `upstream_dependency_hash` when a governing markdown file exists.

## 2. Reliability Metrics Tracking

Write aggregated telemetry to `SDLC_METRICS.json` in the workspace root after every execution loop. Do not leave metrics as unnamed context variables. Do not invent coverage. Copy the line ratio from the `cargo llvm-cov --fail-under-lines 80` summary after a real run in `backend/`.

```json
{
  "success_rate": {
    "compiled_ok": 0,
    "syntax_failures": 0
  },
  "retry_rollback_frequency": 0,
  "mttr_ms": null,
  "stage1_epoch_ms": null,
  "stage7_epoch_ms": null,
  "end_to_end_latency_ms": null,
  "line_coverage_ratio": null,
  "updated_at": "2026-11-16T00:00:00Z"
}
```

* **Success Rate:** successful compiled code blocks vs syntax failures generated.
* **Retry/Rollback Frequency:** increment on each Gate 4 fallback loop.
* **MTTR:** milliseconds from a failing coverage/test command to a successful re-run of that suite.
* **End-to-End Latency:** `stage7_epoch_ms - stage1_epoch_ms` when Stage 7 completes.
* Leave Stage 7 fields and `line_coverage_ratio` **null** until that measurement actually exists.

## 3. Hard Policy Compliance Guardrails

* **No concatenated SQL:** All MySQL access must use `sqlx::query` or `sqlx::query_as` with `.bind()`. String concatenation or `format!` of SQL is a Gate 4C failure.
* **Secret isolation:** Do not hardcode credentials, production passwords, or API keys in Rust. Read secrets from the environment variables in `TECH_STACK.md`. The dev default `Tiger` is lab-only. Production must inject `DB_PASSWORD` and must not fall back to that default.
* **No test bypass:** Never add `#[ignore]` to make a suite pass. Do not pass a skip flag to Cargo.

## 4. Work Execution Cycle (State Gate Constrained)

1. **Gate 2 lock:** Do not generate functional Rust sources, `backend/Cargo.toml`, `backend/Cargo.lock`, `backend/src/**`, or `backend/sql/**` until Gate 2 is approved. Approval is a workspace file `GATE2_APPROVAL.txt` containing a line of the form `[APPROVED: HUMAN_ENGINEER_ID]`. Documentation under `specs/`, `AGENTS.md`, `.cursor/rules/**`, `Dockerfile`, and `docker-compose.yml` may exist before Gate 2; they do not unlock Stage 3.
2. **Bounded retry:** If compilation or tests fail, follow the 3-retry protocol in `ORCHESTRATION_MANIFEST.md`. Fix production or test code so assertions pass. Do not weaken coverage below 80%.
3. **Lineage:** After each mutation, append to `SDLC_AUDIT_LOG.json` with the SHA-256 of the markdown file that justified the change (`PRD.md`, `SYSTEM_DESIGN.md`, `TECH_STACK.md`, or `TESTING_PLAN.md`).
