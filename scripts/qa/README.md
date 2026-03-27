# QA shared host (DEX + bridge on one machine)

## Port conflict (LocalTerra)

If **cl8y-dex-terraclassic** (or anything else) already uses **26657 / 1317 / 9090 / 9091**, the bridge stack cannot bind the same host ports. Use remapped ports via `scripts/qa/qa-host.env` (defaults: **26658**, **1318**, **9092**, **9093**).

**Option A — append to repo-root `.env`** (Docker Compose reads it):

```bash
cd /srv/qa/repos/cl8y-bridge-monorepo
cat >> .env <<'EOF'
E2E_TERRA_RPC_PORT=26658
E2E_TERRA_LCD_PORT=1318
E2E_TERRA_GRPC_PORT=9092
E2E_TERRA_GRPC_WEB_PORT=9093
EOF
```

**Option B — rely on `make start-qa`**: it sources `scripts/qa/qa-host.env` before `docker compose up`, which exports the same `E2E_TERRA_*` values for compose interpolation.

## One command on the QA server

1. One-time: `cp packages/operator/.env.example .env` at repo root and fill **DATABASE_URL**, **EVM_PRIVATE_KEY**, **TERRA_MNEMONIC**, **SOLANA_PRIVATE_KEY** (local dev values from operator README).

2. **sqlx CLI** (for `make operator-migrate` / `make start-qa`): install once per machine:

   ```bash
   cargo install sqlx-cli --no-default-features --features rustls,postgres
   ```

   Ensure **`sqlx`** is on **`PATH`** (usually `~/.cargo/bin`). If you only have the Cargo subcommand, **`cargo sqlx migrate run`** is used automatically by **`scripts/operator-migrate.sh`**.

3. Run:

```bash
make start-qa
```

**Before** starting, this stops any existing canceler, operator, and runs **`docker compose down`** so you always get a clean bring-up.

It then starts Docker (Anvil, LocalTerra, Solana, Postgres), runs migrations, **`make deploy`** (EVM/Terra deploy scripts **merge** bridge addresses into existing **`.env`** files; **`deploy-solana`** runs **`scripts/solana/airdrop-for-anchor-deploy.sh`** so the Anchor deploy keypair is funded on localnet), writes **`.env.e2e.local`** and **`packages/frontend/.env.local`**, starts **operator** and **canceler**, verifies **`/health`**, and prints a **copy-paste `ssh -N -L …`** command for your laptop (ports come from **`scripts/qa/qa-host.env`**). Optionally set **`QA_SSH_DEST=user@host`** in the environment when running **`make start-qa`** to bake your SSH login into that command instead of **`$(whoami)@$(hostname -f)`**.

4. Stop:

```bash
make stop-qa
```

## Laptop + SSH (still required)

The UI runs in a browser on the laptop. **`make start-qa`** prints the exact **`ssh -N -L … user@host`** block at the end (after health checks). Use that command on your laptop, or set **`QA_SSH_DEST=brouie-cl8y-qa@your-host`** when running **`make start-qa`** on the server so the printed destination matches your account.

Then open the app at `http://localhost:5173` with `npm run dev` locally (`.env.local` should use `127.0.0.1` — `make start-qa` generates matching `packages/frontend/.env.local` on the server; copy or sync to your laptop).

## Operator API port

On a shared host, **Terra gRPC** may use host port **9092**. The operator API defaults to **9092** in code; `qa-host.env` sets **OPERATOR_API_PORT=9094** so they do not collide.
