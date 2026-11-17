# Technology Stack Profile: Rust URL Shortener (Axum & MySQL)

This document is the **only** allowed backend toolchain for v1. Crates not listed here are high-impact (see `ORCHESTRATION_MANIFEST.md` §5.1).

**Explicitly out of v1 (do not add):** Redis, an auth or CSRF crate, a rate-limiter, Safe Browsing clients, a migration crate (no sqlx-cli migrate, Refinery, or SeaORM), utoipa, ammonia, a metrics/actuator endpoint. Schema is applied by executing `backend/sql/schema.sql` as described below, plus the DDL in `SYSTEM_DESIGN.md`.

**Frontend:** Node.js 24 and React are specified in `specs/frontend/TECH_STACK.md`. They are not dependencies of `backend/Cargo.toml`.

## 1. Core Tooling Versions

* **Language:** Rust stable, edition **2024**. Toolchain on this machine: rustc **1.100.0**, cargo **1.100.0**, rustup **1.29.3**. Host `x86_64-pc-windows-msvc`.
* **HTTP:** Axum **0.8.9** on Tokio 1
* **Build:** Cargo. Crate name `url-shortener`. Module root `backend/` (`backend/Cargo.toml`)
* **Database:** SQLx **0.9.0**, features `runtime-tokio`, `mysql`, `mysql-rsa`, `chrono` (`default-features = false`). `mysql-rsa` is required for MySQL 8’s `caching_sha2_password`.
* **Test DB:** Testcontainers MySQL **8.0** (dev-dependency). Tests must not use `shortener_dev` or `shortener_prod`.
* **Coverage:** `cargo-llvm-cov` (installed beside the toolchain, not as a library dependency). **Minimum 80% line coverage.** `cargo llvm-cov --fail-under-lines 80` fails below that bar.

## 2. Crate allow-list (`backend/Cargo.toml`)

```toml
[package]
name = "url-shortener"
version = "0.1.0"
edition = "2024"

[dependencies]
axum = "0.8.9"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.9.0", default-features = false, features = ["runtime-tokio", "mysql", "mysql-rsa", "chrono"] }
chrono = { version = "0.4", default-features = false, features = ["clock", "std", "serde"] }
thiserror = "2"
url = "2"
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
testcontainers = "0.28.0"
```

`testcontainers` 0.28.0 is dev-only. Start image `mysql:8.0` with that crate. Do not add `testcontainers-modules` or other crates.

SQL is `sqlx::query` / `sqlx::query_as` with `.bind()`. Do not use the compile-time `query!` macro (it needs a database at compile time). Do not interpolate values into SQL strings.

## 3. Configuration

There is no `application.yml`. The process reads environment variables. Lab defaults below apply only when `APP_ENV=dev`. Production has **no** password default: a missing `DB_PASSWORD` in prod is a startup failure. Never commit a literal production password in Rust.

`APP_BASE_URL` builds `shortUrl` in API responses. It has no trailing slash.

| Variable | Dev default | Prod |
|:---------|:------------|:-----|
| `APP_ENV` | `dev` | `prod` |
| `APP_BASE_URL` | `http://localhost:8080` | required |
| `BIND_ADDR` | `0.0.0.0:8080` | `0.0.0.0:8080` |
| `DB_HOST` | `localhost` | required |
| `DB_PORT` | `3306` | `3306` |
| `DB_NAME` | `shortener_dev` | `shortener_prod` |
| `DB_USER` | `root` | required |
| `DB_PASSWORD` | `Tiger` (lab only) | required, no default |

Build the SQLx URL from those parts. Set the session time zone to UTC and the connection collation to `utf8mb4_unicode_ci`.

* **dev:** on startup, run `backend/sql/schema.sql`. Ignore “already exists” on restart. Do not `ALTER` the table afterward.
* **prod:** do not run `schema.sql`. Compose or a DBA applies it, including `AUTO_INCREMENT=56800235`.
* **test:** `APP_ENV=test`. Integration tests start MySQL 8.0 with Testcontainers, create the schema from the same file, and build the Axum router against that URL. Do not point tests at a developer’s local `shortener_dev`.

`docker-compose.yml` (workspace root, Stage 6) must pass `DB_HOST`, `DB_NAME`, `DB_USER`, `DB_PASSWORD`, and `APP_BASE_URL` into the API service. Compose may set lab passwords in `environment:`. Rust reads only the variables.

## 4. Development Constraints & Coding Standards

* **API types:** Request and response bodies are Serde structs in `backend/src/web/dto.rs` with `#[serde(rename_all = "camelCase")]`. `ttlInDays` is `Option<i64>`. Do not add a builder crate.
* **Rows:** a plain struct in `backend/src/repo/url_mapping.rs`. The repository is a trait so service tests can use an in-memory fake.
* **Database access:** the sqlx implementation of that trait is the only production database code. Native SQL built with `format!`, `+`, or string interpolation is banned. Bind parameters are required, including the batched expiry delete.
* **Scheduling:** a Tokio interval task started from `main`. No extra scheduler crate.
* **Layout when the crate is generated:**

```text
backend/
├── Cargo.toml
├── sql/schema.sql
└── src/
    ├── main.rs
    ├── config.rs
    ├── encoding/base62.rs
    ├── error.rs
    ├── repo/url_mapping.rs
    ├── service/url_shortener.rs
    ├── schedule/cleanup.rs
    └── web/
        ├── dto.rs
        ├── shorten.rs
        └── redirect.rs
```
