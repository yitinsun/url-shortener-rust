# 03 — Public access demo and HTML sanitization

Brownfield playbook. Run after `02-legal-paths.md`. This is a **demo**, not product login. No users, JWT, or login page.

The Java reference used Spring Security (`permitAll`, CSRF off) and jsoup 1.18.1. This crate has neither. The same behavior is:

1. A `PublicAccessLayer` that documents the routes as public, adds no `Authorization` challenge, and never returns **401**.
2. An `HtmlSanitizeLayer` that rejects HTML/script in `longUrl` using **ammonia 4.2.5** (the HTML sanitizer on the allow-list for this playbook).

Both are outside `TECH_STACK.md`. Wait for a new human signature before editing `backend/Cargo.toml`. ammonia is the only new crate. Do not add an auth framework.

On a finding, return the existing **400** JSON from the error type. The `message` mentions sanitization or HTML. Do not invent a second error shape.

`LegalPathLayer` stays the allowlist and runs **first**. This layer runs after it, so illegal URLs still get HTML 404.

## Cargo

```toml
ammonia = "4.2.5"
```

Do not add OAuth2 or JWT crates.

## Public access

`backend/src/web/public_access.rs`:

- Do not require a CSRF token, so `POST /api/v1/shorten` from Swagger and curl still works.
- Do not add form login, HTTP basic, or a logout route.
- Documented routes (`/api/v1/shorten`, `/short/**`, Swagger, `/v3/api-docs/**`) are public.
- Unknown paths still reach `LegalPathLayer` (HTML 404, not 401).

Requiring authentication is a later re-plan, not this demo.

## Ammonia

Buffer `POST` JSON so the handler can still read the body. Treat markup as unsafe. A plain `https://example.com/path` must pass.

Reject rather than silently clean. Apply only to `longUrl` on `POST /api/v1/shorten`.

Unsafe when `ammonia::Builder::empty().clean(value)` is not equal to `value`, or when the fragment parses to one or more HTML elements. That is the same reject-or-keep decision the reference service made with jsoup’s empty safelist.

## Tests

- Existing API tests still pass (POST is not blocked).
- `longUrl` containing HTML or `<script>` → **400** JSON whose message mentions sanitization or HTML.
- `HtmlSanitizer` unit tests: clean URL vs `<script>` / `<img>`.

## Files

| File | Role |
|:-----|:-----|
| `backend/Cargo.toml` | ammonia 4.2.5 |
| `web/public_access.rs` | public routes, no 401 |
| `web/html_sanitizer.rs` | `looks_unsafe` |
| `web/html_sanitize.rs` | layer after the allowlist |
| `error.rs` | map the finding to 400 JSON |
