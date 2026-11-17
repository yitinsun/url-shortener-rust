# Brownfield scenario

**Intent:** change the running backend after Gate 4 without skipping the dependency allow-list, tests, or error pages. This file is an evaluation index. The playbooks generate the changes.

**Decomposition:** one playbook → impact → code and tests → `cargo llvm-cov --fail-under-lines 80` from `backend/`. A new Cargo dependency or a table change waits for a human signature.

| Order | Change | Playbook | Modules | API / data |
|:------|:-------|:---------|:--------|:-----------|
| 1 | OpenAPI UI | `01-add-swagger.md` | `backend/Cargo.toml`, utoipa document, DTO schemas | `/swagger-ui.html`, `/v3/api-docs` |
| 2 | Allowlist and HTML 404 for illegal paths | `02-legal-paths.md` | `LegalPathLayer`, `LegalPaths`, friendly HTML | Fence around the greenfield routes. Redirect stays `GET /short/{code}` |
| 3 | Public access + HTML sanitizer | `03-security-jsoup.md` | `PublicAccessLayer`, `HtmlSanitizeLayer`, ammonia 4.2.5 | 400 JSON if `longUrl` contains HTML. Routes stay public (no 401) |
| 4 | Docs and measured metrics | `04-refine.md` | README, audit log, scenario outputs | No schema |

**Orchestration path:** post–Gate 4, one playbook at a time. Greenfield already specifies `GET /short/{shortCode}`. Do not reopen that path as a second design.

**Validation:** after each playbook, the greenfield API tests still pass, plus the cases named in that playbook. Command remains `cargo llvm-cov --fail-under-lines 80`.

**Outputs in the workspace:**

- Swagger: `utoipa` 6.0.0 and `utoipa-swagger-ui` 10.0.2 in `backend/Cargo.toml`. Document routes are `GET /swagger-ui.html`, `GET /swagger-ui/**`, `GET /v3/api-docs`, and `GET /v3/api-docs.yaml`. `GET /short/{shortCode}` stays out of the OpenAPI document.
- Allowlist: `backend/src/web/legal_path.rs` (`LegalPaths`, `LegalPathLayer`).
- Public-access demo and ammonia sanitizer: `backend/src/web/public_access.rs`, `backend/src/web/html_sanitizer.rs`, `backend/src/web/html_sanitize.rs`, and `ammonia` 4.2.5 in `backend/Cargo.toml`.
- Docs: `README.md`, `SDLC_AUDIT_LOG.json`, `SDLC_METRICS.json` (`line_coverage_ratio` `0.9121` from the 2026-11-17T13:43:24Z llvm-cov run).
- Client: `frontend/` (React 19, Vite 6). It calls `POST /api/v1/shorten` only. No coverage percentage is defined for the client.
- No schema change. No actuator. No click-count column.
- No container image. `backend/Dockerfile` and root `docker-compose.yml` are absent. Stage 7 is open.
