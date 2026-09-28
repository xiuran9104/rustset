# AI Startup Guide

This file is the handoff guide for AI coding agents working in this repository. Read it before starting services or changing deployment docs.

## Project Shape

- Backend: Rust workspace, gateway entrypoint at `services/gateway`.
- Frontend: Vue 3 / Vben Admin (Ant Design Vue) monorepo at `apps/web`, managed with bun.
- Database migrations: `sql/postgresql`, executed automatically by the Rust gateway on startup. `0001_initial.sql` is the consolidated schema and baseline data.
- Bootstrap reference: `sql/bootstrap/current.sql` is a reference-only `pg_dump` snapshot and is never loaded by the application. The migration chain is sufficient to initialize a new server without `current.sql`.
- Local infrastructure: PostgreSQL, Redis, NATS, and RustFS via `script/docker/docker-compose.yml`.

Do not mount `sql/postgresql` into PostgreSQL init scripts. The gateway owns database initialization through SQLx, and PostgreSQL should start as an empty database.

## Database Change Workflow

Whenever a database schema or baseline-data change is made:

1. Add a new numbered migration under `sql/postgresql`; never edit a migration that has already been released or applied.
2. Make the migration idempotent so both upgraded databases and empty-database bootstrap are supported.
3. Run `bash script/test-database-migrations.sh` to prove an empty PostgreSQL instance reaches the latest schema and baseline data without importing `current.sql`.
4. After applying all migrations to a clean reference database, export a fresh `sql/bootstrap/current.sql` with `pg_dump` for review and comparison. The gateway must remain fully functional when this snapshot is absent.
5. Update the expected migration count and relevant baseline assertions in `crates/framework/database/tests/migrations.rs`.

The migration history was intentionally reset to the consolidated `0001_initial.sql`; existing databases must be recreated once. After this reset is released, do not rewrite `0001` or any subsequently applied migration.

## Local Development Startup

From the repository root:

```bash
docker compose -f script/docker/docker-compose.yml up -d
```

Start the backend gateway:

```bash
export DATABASE_URL='postgres://rustset:rustset@127.0.0.1:5432/rustset'
export REDIS_URL='redis://127.0.0.1:6379'
export JWT_SECRET='local-development-jwt-secret-change-me-32bytes'
export RUST_LOG='info'
cargo run -p rustset-gateway
```

In a second terminal, start the Vben frontend:

```bash
cd apps/web
bun install
bun run dev:antd
```

Open:

- Frontend (dev): `http://127.0.0.1:5666`
- Backend health: `http://127.0.0.1:8080/health`
- OpenAPI: `http://127.0.0.1:8080/openapi.json`
- RustFS console: `http://127.0.0.1:9001/rustfs/console/`

The compose file exposes the RustFS S3 API at `http://127.0.0.1:9000`. Local credentials default to `rustset` / `rustset_password`; override them with `RUSTFS_ACCESS_KEY` and `RUSTFS_SECRET_KEY` before starting Compose. The gateway uses these variables plus `RUSTFS_ENDPOINT`, `RUSTFS_REGION`, and `RUSTFS_BUCKET` to configure S3-compatible object storage. It creates the bucket at startup when missing. These defaults are for local development only; non-local endpoints require explicit credentials. Browser direct-upload mode also needs an `RUSTFS_ENDPOINT` reachable from the browser. Single-file uploads default to a 50 MiB cap controlled by `INFRA_UPLOAD_MAX_BYTES`. Migrate legacy local files with `bash script/migrate-local-uploads-to-rustfs.sh`; it deletes local copies only after validating all matching RustFS objects.

Default local bootstrap account (seeded by the RustSet migration baseline):

- Username: `admin`
- Password: `admin123`

`BOOTSTRAP_ADMIN_USERNAME` / `BOOTSTRAP_ADMIN_PASSWORD` are the recovery/initialization path:

- When no enabled super administrator exists, startup creates the named account (in the lowest enabled tenant, linked to the `super_admin` role — re-enabling or creating that role if needed) and logs `bootstrapped initial administrator`.
- When the named account exists with an empty password column, startup sets the password from `BOOTSTRAP_ADMIN_PASSWORD` (conditional update on `password = ''`; a changed password is never overwritten).

Change the seeded password after first login on any real deployment.

## Local Verification

Use these checks after startup:

```bash
curl -fsS http://127.0.0.1:8080/health
cargo test --workspace
bash script/test-database-migrations.sh
```

Frontend build check:

```bash
cd apps/web
bun install
bun run check:type
bun run build:antd
```

## New Server Startup

Install prerequisites:

- Rust stable with Rust 2024 edition support.
- Docker and Docker Compose.
- Nginx or another reverse proxy for production frontend/API routing.

Clone and enter the repository:

```bash
git clone <repo-url> rustset
cd rustset
```

Start infrastructure. For a single-server deployment, the repository compose file is enough:

```bash
docker compose -f script/docker/docker-compose.yml up -d
```

For managed PostgreSQL or Redis, skip those compose services and set `DATABASE_URL` / `REDIS_URL` to the managed endpoints.

Create a backend environment file outside git, for example `/etc/rustset/gateway.env`:

```bash
DATABASE_URL=postgres://rustset:rustset@127.0.0.1:5432/rustset
REDIS_URL=redis://127.0.0.1:6379
JWT_SECRET=replace-with-a-strong-random-secret-at-least-32-bytes
GATEWAY_HOST=0.0.0.0
GATEWAY_PORT=8080
RUST_LOG=info
BOOTSTRAP_ADMIN_USERNAME=admin
BOOTSTRAP_ADMIN_PASSWORD=replace-with-a-strong-initial-password
```

Build the backend:

```bash
cargo build --release -p rustset-gateway
```

Run once manually to verify migrations and initial admin creation:

```bash
set -a
. /etc/rustset/gateway.env
set +a
./target/release/rustset-gateway
```

After the first successful login, remove `BOOTSTRAP_ADMIN_PASSWORD` from `/etc/rustset/gateway.env` and restart the service.

## systemd Service

Use systemd or another process manager in production. Example `/etc/systemd/system/rustset-gateway.service`:

```ini
[Unit]
Description=RustSet Gateway
After=network-online.target docker.service
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/opt/rustset
EnvironmentFile=/etc/rustset/gateway.env
ExecStart=/opt/rustset/target/release/rustset-gateway
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now rustset-gateway
sudo systemctl status rustset-gateway
curl -fsS http://127.0.0.1:8080/health
```

## Frontend Production

Build the Vben frontend (bun required):

```bash
cd apps/web
bun install
bun run build:antd
```

Static output is in `apps/web/apps/web-antd/dist` (plus a `dist.zip` archive).

Serve that directory through Nginx or a CDN. Route SPA paths to `index.html`, and reverse proxy API traffic to the gateway.

Minimal Nginx location rules:

```nginx
location / {
    try_files $uri $uri/ /index.html;
}

location /api/ {
    proxy_pass http://127.0.0.1:8080/;
    proxy_http_version 1.1;
    proxy_buffering off;
    proxy_read_timeout 600s;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
}
```

## Common Problems

- `DATABASE_URL is required`: export it or add it to the systemd environment file.
- JWT startup error: `JWT_SECRET` must be at least 32 bytes.
- No admin account: set `BOOTSTRAP_ADMIN_USERNAME` / `BOOTSTRAP_ADMIN_PASSWORD` and restart; startup creates or repairs the administrator when none is enabled.
- Frontend install fails under pnpm: the project is bun-managed; install bun (`curl -fsSL https://bun.sh/install | bash`) and use `bun install`.
- Frontend API 404: check that the gateway is running and Nginx `/api/` proxy prefix handling.
- SSE responses arrive all at once: disable proxy buffering and increase read timeout.
- Port already in use: check `ss -ltnp | rg ':(8080|5666|5432|6379)'`.
