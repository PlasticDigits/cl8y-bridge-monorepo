# QA shared host (DEX + bridge on one machine)

How this doc is organized:

1. **[On the QA server](#on-the-qa-server)** — one-time prep and **`make start-qa`**
2. **[On your laptop](#on-your-laptop)** — SSH tunnel, copy **`local.env`**, generate frontend env, run Vite (same order as **`make start-qa`** prints at the end)
3. **[Troubleshooting](#troubleshooting)** — Settings vs bridge page, LocalTerra LCD, etc.
4. **[Appendix: port conflict (LocalTerra)](#appendix-port-conflict-localterra)**
5. **[Appendix: operator API port](#appendix-operator-api-port)**

---

## On the QA server

### One-time setup

1. **Repo-root `.env`:** `cp packages/operator/.env.example .env` and set at least **DATABASE_URL** (e.g. `postgres://operator:operator@127.0.0.1:5433/operator`) and **SOLANA_PRIVATE_KEY** (base58 from the Anchor keypair, usually `~/.config/solana/id.json`). With **`scripts/qa/qa-host.env`** and **`QA_SHARED_HOST=1`**, typical Anvil/Terra fields are filled so a minimal `.env` is often enough.

2. **sqlx CLI** (for migrations / **`make start-qa`**):

   ```bash
   cargo install sqlx-cli --no-default-features --features rustls,postgres
   ```

   Put **`sqlx`** on **`PATH`** (often `~/.cargo/bin`). If missing, **`cargo sqlx migrate run`** is used via **`scripts/operator-migrate.sh`**.

### Each bring-up: `make start-qa`

```bash
make start-qa
```

**Before** starting, this stops canceler/operator and runs **`docker compose down`** for a clean stack.

It then: starts Docker (Anvil, LocalTerra, Solana, Postgres) → migrations → **`make deploy`** (writes bridge addresses into **`.deploy/local.env`**, merges into operator **`.env`**) → writes **`.env.e2e.local`** and **`packages/frontend/.env.local` on the server** → starts operator + canceler → health checks → **prints [laptop workflow steps](#on-your-laptop)** (SSH, **`scp`**, **`write-frontend-env-local.sh`**, **`npm run dev`**).

**Optional — bake your SSH login into that printed block** (set on the server when you run **`make start-qa`**):

| Variable | Purpose |
|----------|---------|
| **`QA_SSH_DEST`** | **`user@host`** as seen from the laptop (default: **`$(whoami)@$(hostname -f)`**) |
| **`QA_SSH_PORT`** | If SSH is not on port 22; the printed **`ssh`** / **`scp`** lines include **`-p`** / **`-P`** |

### Stop the QA stack

```bash
make stop-qa
```

---

## On your laptop

Do these **in order** after the QA server has finished **`make start-qa`** successfully. The same steps are printed at the end of **`make start-qa`** so you can copy-paste.

### Step 1 — SSH port forwards

On the **laptop**, run the **`ssh -4 -N`** block from the **`make start-qa`** output. Leave that terminal open. Using **`127.0.0.1`** on both sides avoids some IPv6 **`[::1]`** bind issues.

This forwards the bridge RPC/LCD/wallet ports from the QA host’s loopback to yours.

### Step 2 — Copy **`.deploy/local.env`** from the QA host

From your **laptop repo clone** (repo root):

```bash
mkdir -p .deploy
scp PATH_FROM_START_QA_OUTPUT
```

Use the exact **`scp`** line from **`make start-qa`** (it includes **`USER@HOST`**, optional **`-P`**, and the full path to **`.deploy/local.env`** on the server).

This file holds **bridge contract addresses** (EVM, Terra, Solana program id) produced by deploy.

### Step 3 — Generate **`packages/frontend/.env.local`**

```bash
./scripts/qa/write-frontend-env-local.sh
```

Or: **`npm run env:local --prefix packages/frontend`**

This merges **`qa-host.env`** (remapped Terra ports, same as Docker/SSH) with **`.deploy/local.env`**. It writes **`VITE_*`** URLs **and** bridge addresses.

- **URLs only** (e.g. Settings checks before you have **`local.env`**): **`./scripts/qa/write-frontend-env-local.sh --urls-only`** — bridge addresses stay empty; the transfer page will not list all chains until you complete Step 2 and re-run **without** **`--urls-only`**.

### Step 4 — Install and run the frontend

```bash
cd packages/frontend && npm ci && npm run dev
```

### Step 5 — Open the app

Open the URL Vite prints (often **`http://localhost:5173`**).

---

## Troubleshooting

| Symptom | What to do |
|--------|------------|
| **Settings OK, bridge page only Solana / no tokens** | You need full **`write-frontend-env-local.sh`** (not **`--urls-only`**) after Step 2 so **`VITE_EVM_BRIDGE_ADDRESS`**, **`VITE_TERRA_BRIDGE_ADDRESS`**, **`VITE_SOLANA_PROGRAM_ID`** are set. Restart Vite. |
| **LocalTerra “Failed to fetch” or LCD shows `:1317`** | Regenerate **`.env.local`** after Step 2; confirm the script logs **`LCD=http://127.0.0.1:1318`** (shared QA remapping). Restart **`npm run dev`**. |
| **`scp` fails** | Use **`QA_SSH_DEST`** / **`QA_SSH_PORT`** when re-running **`make start-qa`** on the server so the printed **`scp`** matches how you SSH. |

---

## Appendix: port conflict (LocalTerra)

If another stack already uses **26657 / 1317 / 9090 / 9091**, the bridge uses remapped ports via **`scripts/qa/qa-host.env`** (defaults **26658**, **1318**, **9092**, **9093**).

**Option A — append to repo-root `.env`** (Docker Compose reads it):

```bash
cd /path/to/cl8y-bridge-monorepo
cat >> .env <<'EOF'
E2E_TERRA_RPC_PORT=26658
E2E_TERRA_LCD_PORT=1318
E2E_TERRA_GRPC_PORT=9092
E2E_TERRA_GRPC_WEB_PORT=9093
EOF
```

**Option B:** Rely on **`make start-qa`**, which sources **`scripts/qa/qa-host.env`** before **`docker compose up`**.

---

## Appendix: operator API port

On a shared host, Terra gRPC may use host port **9092**. The operator API defaults to **9092** in code; **`qa-host.env`** sets **`OPERATOR_API_PORT=9094`** so they do not collide.
