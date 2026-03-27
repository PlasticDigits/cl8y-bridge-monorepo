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

1. One-time: `cp packages/operator/.env.example .env` at repo root and set at least **DATABASE_URL** (e.g. `postgres://operator:operator@127.0.0.1:5433/operator`) and **SOLANA_PRIVATE_KEY** (base58 from the same keypair Anchor uses, usually `~/.config/solana/id.json`). **`scripts/qa/qa-host.env`** fills typical local Anvil/Terra fields when **`QA_SHARED_HOST=1`** (including **EVM_CHAIN_ID**, **FEE_RECIPIENT**, test **TERRA_MNEMONIC**, etc.) so a minimal `.env` is enough.

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

It then starts Docker (Anvil, LocalTerra, Solana, Postgres), runs migrations, **`make deploy`** (EVM/Terra deploy scripts **merge** bridge addresses into existing **`.env`** files; **`deploy-solana`** runs **`scripts/solana/airdrop-for-anchor-deploy.sh`** so the Anchor deploy keypair is funded on localnet), writes **`.env.e2e.local`** and **`packages/frontend/.env.local`**, starts **operator** and **canceler**, verifies **`/health`**, and prints a **copy-paste `ssh -4 -L …`** command for your laptop (ports come from **`scripts/qa/qa-host.env`**). When running **`make start-qa`** on the server, you can set:

- **`QA_SSH_DEST`** — value passed to **`ssh`** / **`scp`** (e.g. `user@hostname` or `user@address`). Default: **`$(whoami)@$(hostname -f)`**.
- **`QA_SSH_PORT`** — only if SSH is not on port 22; the printed commands include **`ssh -p`** and **`scp -P`** accordingly.

4. Stop:

```bash
make stop-qa
```

## Laptop + SSH (still required)

The UI runs in a browser on the laptop. **`make start-qa`** prints the exact **`ssh -4 -L 127.0.0.1:…`** block at the end. Use that on your laptop, or set **`QA_SSH_DEST`** / **`QA_SSH_PORT`** on the server when running **`make start-qa`** so the printed `scp`/`ssh` lines match how you connect.

**Frontend env (no manual `VITE_*` sync):** URLs for QA shared-host are fixed in **`scripts/qa/qa-host.env`** (same ports as Docker / SSH forwards). Contract addresses live in **`.deploy/local.env`** after deploy. On your laptop clone:

```bash
mkdir -p .deploy
scp USER@QA_HOST:/path/to/cl8y-bridge-monorepo/.deploy/local.env .deploy/local.env
# If your SSH server uses a non-default port, add the usual scp -P / ssh -p flags to match.
./scripts/qa/write-frontend-env-local.sh
# No .deploy/local.env yet (only need correct LCD/RPC for SSH + Settings): ./scripts/qa/write-frontend-env-local.sh --urls-only
# or: npm run env:local --prefix packages/frontend
# or: npm run env:local --prefix packages/frontend -- --urls-only
cd packages/frontend && npm ci && npm run dev
```

Then open `http://localhost:5173`. You only copy **one small file** from the server, not a hand-edited `.env.local`.

**Bridge page only lists Solana (or no tokens):** the transfer UI only shows chains that have **`VITE_*` bridge contract addresses** (EVM, Terra, Solana program id). **`--urls-only`** / RPC-only **`.env.local`** is not enough. Copy **`.deploy/local.env`** from the server and run **`./scripts/qa/write-frontend-env-local.sh`** without **`--urls-only`**, then restart Vite.

**LocalTerra shows `localhost:1317` or “Failed to fetch”:** Settings uses **`VITE_TERRA_LCD_URL`** from **`packages/frontend/.env.local`**. On shared QA, that must be the **remapped** LCD (**1318**), not **`.env.example`**’s **1317**. Run **`./scripts/qa/write-frontend-env-local.sh`** (it logs **`LCD=...`**), then **restart** **`npm run dev`**. A copy of **`.env.example`** → **`.env`** alone is not enough unless **`.env.local`** overrides Terra URLs.

## Operator API port

On a shared host, **Terra gRPC** may use host port **9092**. The operator API defaults to **9092** in code; `qa-host.env` sets **OPERATOR_API_PORT=9094** so they do not collide.
