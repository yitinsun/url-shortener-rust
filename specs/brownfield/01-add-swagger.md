# 01 — Swagger UI (utoipa)

Brownfield playbook. Run only after Gate 4 is green. Adding these crates is **high impact**: they are not in `specs/greenfield/TECH_STACK.md`. Wait for a new human signature before editing `backend/Cargo.toml`.

Swagger publishes the HTTP API that already exists. It does not change status codes, the schema, or redirect behavior. The written contract stays `specs/contracts/openapi.yaml`.

**Crates:** `utoipa` **6.0.0** and `utoipa-swagger-ui` **10.0.2** with the `axum` feature. Serve the same paths the legal-path playbook allowlists.

| Item | Value |
|:-----|:------|
| Crate | `backend/` (`url-shortener`) |
| Axum | 0.8.9 |
| Rust | stable, edition 2024 |
| JSON API | `POST /api/v1/shorten` |
| Redirect | `GET /short/{shortCode}` → **302** (omit from the document) |

After enablement: `http://localhost:8080/swagger-ui.html` and `http://localhost:8080/v3/api-docs`.

## 1. Cargo

```toml
utoipa = { version = "6.0.0", features = ["chrono"] }
utoipa-swagger-ui = { version = "10.0.2", features = ["axum"] }
```

Check with `cargo tree -p utoipa -p utoipa-swagger-ui` from `backend/`.

## 2. Paths

Mount Swagger UI at `/swagger-ui.html` (and the assets it requests under `/swagger-ui/`). Mount the generated document at:

* `GET /v3/api-docs` — OpenAPI JSON
* `GET /v3/api-docs.yaml` — OpenAPI YAML

Both are off unless enabled. Dev default is on. Prod default is off unless the env var is set:

| Variable | Dev default | Prod default |
|:---------|:------------|:-------------|
| `OPENAPI_ENABLED` | `true` | `false` |

The document includes only `/api/v1/**`. Do not register `GET /short/{shortCode}` on the OpenAPI spec.

## 3. Document metadata

Build the `utoipa::OpenApi` value with title **URL Shortener API**, version `0.1.0`, and a server URL from `APP_BASE_URL`. The value is documentation only.

Do not add a Bearer/JWT security scheme. The create API stays public.

## 4. Types

Derive `ToSchema` on `ShortenRequest`, `ShortenResponse`, and `ErrorResponse`. Path and status annotations must not change paths, status codes, or validation. Document **201**, **400**, and **503** on `POST /api/v1/shorten` as in `openapi.yaml`.

## 5. What this playbook does not do

- No auth middleware in this change (`03-security-jsoup.md` is separate).
- No schema change.
- No replacement for `tests/url_shortener_api.rs`.

## 6. Smoke check

1. Start the backend on port 8080.
2. `GET /swagger-ui.html` shows **URL Shortener API** `0.1.0`.
3. The catalog lists `POST /api/v1/shorten` and omits `GET /short/{shortCode}`.
4. Try-it-out with a valid `https://…` URL returns **201**.
5. `GET /v3/api-docs` is OpenAPI 3 JSON.
6. `cargo llvm-cov --fail-under-lines 80` from `backend/` stays green.
