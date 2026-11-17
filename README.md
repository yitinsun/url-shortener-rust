# URL shortener

This project is implemented by Rust + React. The React client shows that sentence in the header.

## Toolchain

- Backend: Rust stable, edition 2024, rustc 1.100, Cargo. Crate root: `backend/`.
- Client: Node.js 24, React 19, TypeScript, Vite 6. Module root: `frontend/`.
- Database: MySQL 8.0.
- Docker is optional. This tree has no `backend/Dockerfile` and no root `docker-compose.yml`.

## Run the backend

Dev defaults are in `specs/greenfield/TECH_STACK.md`. `APP_ENV=dev` listens on `0.0.0.0:8080`, uses `APP_BASE_URL=http://localhost:8080`, and applies `backend/sql/schema.sql` to MySQL on `localhost:3306` database `shortener_dev`. The lab `DB_PASSWORD` default is `Tiger`. Production must set `DB_PASSWORD` itself.

```powershell
cd backend
cargo run
```

## Run the React client

The dev server is `http://localhost:5173`. Requests to `/api` are proxied to `http://localhost:8080`.

```powershell
cd frontend
npm ci
npm run dev
```

`npm run build` typechecks and builds the client. `npm test` runs Vitest.

## Tests

From `backend/`:

```powershell
cargo llvm-cov --fail-under-lines 80
```

The gate requires every test to pass and line coverage of at least 80%. The measured run on 2026-11-17T13:43:24Z is 91.21% (1286 of 1410 lines).

## HTTP

- `POST /api/v1/shorten` creates a short URL and returns 201 JSON.
- `GET /short/{code}` redirects with 302.
- `GET /swagger-ui.html` and `GET /v3/api-docs` publish the create API.

## Where the specs live

- `specs/greenfield/` — product, design, toolchain, tests, and the stage graph
- `specs/brownfield/` — Swagger, legal paths, public access and HTML sanitization, then these docs
- `GATE2_APPROVAL.txt` — human signatures
- `SDLC_AUDIT_LOG.json` — lineage
- `SDLC_METRICS.json` — measured coverage and repair counts

## Limits

Codes are sequential Base62 of the MySQL id. The sequence starts at 56800235. The public-access layer is a demo: there is no login and no CSRF challenge. `stage1_epoch_ms` and `stage7_epoch_ms` are null. Stage 7 is open. `docker compose up --build` was run once and exited 1 with `no configuration file provided: not found`. No image is claimed.
