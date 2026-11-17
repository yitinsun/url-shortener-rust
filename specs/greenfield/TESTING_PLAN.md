# Automated Test Verification Suite Plan

Mandatory tests and the agent loop for Gate 4. From `backend/`, the suite must pass `cargo llvm-cov --fail-under-lines 80` at **100%** test success and **line coverage >= 80%**. Never use `#[ignore]`.

Coverage is enforced by `cargo-llvm-cov` as named in `TECH_STACK.md`. Do not invent a percentage in `specs/scenarios/01-greenfield.md` or `SDLC_METRICS.json`. Copy `line_coverage_ratio` from that command’s summary after a real run.

## 1. Unit Testing Matrix

### 1.1 `src/encoding/base62.rs` tests

No Axum router and no database. Assert:

* **Pivot:** `encode(56800235)` is alphanumeric `[0-9A-Za-z]+` and length **>= 5** (and **<= 7** for this pivot).
* **Alphabet:** encoded forms contain no `+` or `/`.
* **Max:** `encode(i64::MAX as u64)` completes without overflow; `decode(encode(i64::MAX as u64))` returns that same value. `BIGINT` ids fit in `u64` for this range; do not use a wrapping 32-bit type.
* **Round-trip:** for a set of random positive ids, `decode(encode(id)) == id`.

### 1.2 `src/service/url_shortener.rs` tests

Use the repository trait with an in-memory fake. No MySQL.

| Case | Setup | Expect |
|:-----|:------|:-------|
| Missing resource | Repo empty for short code | not-found error |
| Expired mapping | `expires_at` in the past | not-found error |
| Idempotent create | Active row for same `longUrl` | No second insert; return existing code |
| Default TTL | `ttlInDays` absent | `expires_at` ≈ now + **730** days |
| Self-loop | `longUrl` host equals `APP_BASE_URL` host | validation error (400 path) |
| Oversize URL | length > 2048 | validation error |

### 1.3 `src/schedule/cleanup.rs` tests

The scheduled delete uses a fixed SQL string with `?` placeholders. Assert a cutoff instant is bound and the statement is not built with `format!` or concatenation.

## 2. HTTP Integration Testing (`tests/url_shortener_api.rs`)

Build the real Axum router and call it with `tower::ServiceExt::oneshot` (or `http_body_util` to read the body). The pool is MySQL 8.0 from Testcontainers, schema from `backend/sql/schema.sql`, including `AUTO_INCREMENT=56800235`. Seed with the repository or with `POST` — not a fake pool.

The error type must serialize the error JSON from `PRD.md` / `SYSTEM_DESIGN.md` for `POST /api/v1/shorten`. Redirect misses use HTML.

### 2.1 `POST /api/v1/shorten`

| Case | Assert |
|:-----|:-------|
| Valid HTTP(S) URL | **201**; body has `shortUrl`, `shortCode`, `expiresAt`; `shortCode` length 5–7; `shortUrl` starts with `{APP_BASE_URL}/short/` |
| Second POST of the same `longUrl` | **201** with the identical `shortCode` |
| `"longUrl": "not-a-valid-url"` | **400** and error JSON `status`/`message` |
| Self-loop to `APP_BASE_URL` | **400** |

### 2.2 `GET /short/{shortCode}`

| Case | Setup | Assert |
|:-----|:------|:-------|
| Active mapping | Persist unexpired row (or create via POST) | **302** and `Location` equals `longUrl` |
| Unknown code | GET a code that was never inserted | **404** friendly HTML (not JSON, not Axum’s plain-text default) |
| Expired mapping | Persist row with `expires_at` in the past | **404** friendly HTML (HTTP layer, not only service unit) |

## 3. Automated Agent Workflow Cycle

`cargo llvm-cov --fail-under-lines 80` is the Gate 4 command. Do not pass `--skip` or `#[ignore]`. The Docker **image build** may skip tests only when Gate 4 has already passed on that source tree.

```
[ Compile ] ──> [ cargo llvm-cov --fail-under-lines 80 ]
                      │
         ┌────────────┴────────────┐
         ▼                         ▼
[ All tests pass + coverage ]   [ Failures or coverage < 80% ]
         │                         │
         ▼                         ▼
[ Gate 5: container phase ]   [ Read compiler / test output ]
         (Stage 6)                 │
                                   ▼
                          [ Patch failing module ]
                                   │
                                   ▼
                          [ Retry; max 3 times ]
                                   │
                    exhausted ──> [ Safe-Stop / rollback to Gate 2 ]
```

This loop **is** the Bounded Retry in `ORCHESTRATION_MANIFEST.md`. Passing tests do **not** rewrite modules. Gate 4C (secrets / parameterized SQL) is a compliance review of the same tree, not an extra test binary — still must pass before Stage 6.
