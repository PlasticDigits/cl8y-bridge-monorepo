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

### Each bring-up: `make start-qa` (alias: `make qa-start`)

```bash
make start-qa
# same as:
make qa-start
```

**Before** starting, this stops canceler/operator and runs **`docker compose down`** for a clean stack.

It then: starts Docker (**anvil**, **anvil1** on 8546, LocalTerra, Solana, Postgres) → migrations → **`make ensure-terra-artifacts`** ( **`bridge.wasm`** via **`make build-terra`** if missing; **`cw20_mintable.wasm`** via **`scripts/ensure-cw20-mintable-wasm.sh`** — **release download first**, else **cw-plus** clone + **`cw20-base`** build with MVP **`RUSTFLAGS`** + optional **`wasm-opt`**, else download again) → **`make deploy`** (EVM + **EVM1** second deploy + Terra with **CW20** + Solana + **`setup-bridge`** with Terra **`register_chain`**) → **`npm ci`** in **`packages/frontend`** when needed (no **`node_modules`**, or **`package-lock.json`** newer than **`node_modules`**) → **`npm run qa:full-token-setup`** in **`packages/frontend`** (same **full E2E token matrix** as Vitest e2e-infra: TokenA/B/C, LUNC, KDEC on both Anvils + Terra, **`registerAllTokens`**, LockUnlock funding, cancel window, canceler registration, **`scripts/solana/register-tokens.sh`**) → merges deploy outputs + **`EVM_CHAINS_COUNT` / `EVM_CHAIN_1_*`** (multi-EVM operator) into repo-root **`.env`** → writes **`.env.e2e.local`** and **`packages/frontend/.env.local`** (including **`VITE_EVM1_*`**) → starts operator + canceler → health checks → **prints [laptop workflow steps](#on-your-laptop)**.

**Manual token step (after `make deploy` only):** `make qa-full-token-setup` or `cd packages/frontend && npm run qa:full-token-setup`.

**Settings → Faucet** rows for dedicated faucet **contracts** (EVM/Terra faucet dApps) are separate from bridge test tokens; they need extra deploy + **`VITE_*_FAUCET_ADDRESS`** if you use that UI. Bridge **token selectors** use registries populated by the QA token setup above.

**Legacy minimal path:** **`make deploy-tokens`** + **`make register-tokens`** (single test ERC20 + bash registration) remains available; **`start-qa`** uses the full matrix instead.

**Optional — bake SSH host/port into that printed block** (set on the server when you run **`make start-qa`**). The printed destination is **`whoami@host`**: **`host`** comes from **`QA_SSH_HOST`** (or this machine’s hostname), and **`whoami`** is whoever runs **`make start-qa`** on the server.

| Variable | Purpose |
|----------|---------|
| **`QA_SSH_HOST`** | Hostname or IP as seen from the laptop (default: this machine’s **`hostname -f`** or **`hostname`**) |
| **`QA_SSH_PORT`** | If SSH is not on port 22; the printed **`ssh`** / **`scp`** lines include **`-p`** / **`-P`** |
| **`START_QA_SKIP_NPM_CI`** | Set to **`1`** to skip the automatic **`npm ci`** in **`packages/frontend`** when **`make start-qa`** would otherwise run it (missing **`node_modules`** or **`package-lock.json`** newer than **`node_modules`**) |

### Stop the QA stack

```bash
make stop-qa
```

---

## On your laptop

Do these **in order** after the QA server has finished **`make start-qa`** successfully. The same steps are printed at the end of **`make start-qa`** so you can copy-paste.

This laptop workflow is for **manual frontend QA** (Vite in the browser). **Automated tests** (Playwright, Vitest bridge/integration, e2e-infra, etc.) should be run **on the QA server** directly—they need services beyond the reduced SSH tunnel (operator, canceler, Postgres, etc.).

### Step 1 — SSH port forwards

On the **laptop**, run the **`ssh -4 -N`** block from the **`make start-qa`** output. Leave that terminal open. Using **`127.0.0.1`** on both sides avoids some IPv6 **`[::1]`** bind issues.

This forwards only the **chain** endpoints the frontend uses (Anvil ×2, LocalTerra RPC/LCD, Solana RPC + WebSocket + faucet). It does **not** include the operator HTTP API or canceler health port—the app talks to contracts via RPC/LCD only. To curl operator/canceler on the laptop, add `-L 127.0.0.1:<port>:127.0.0.1:<port>` manually (see `OPERATOR_API_PORT` / `CANCELER_HEALTH_URL` on the server, often `9094` / `9099`).

### Step 2 — Copy **`.deploy/local.env`** from the QA host

From your **laptop repo clone** (repo root):

```bash
scp PATH_FROM_START_QA_OUTPUT
```

The repo includes an empty **`.deploy/`** directory (via **`.deploy/.gitkeep`**) so **`scp … .deploy/local.env`** works without **`mkdir`**. Use the exact **`scp`** line from **`make start-qa`** ( **`USER@HOST`**, optional **`-P`**, path to **`.deploy/local.env`** on the server).

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
| **Settings OK, bridge page only Solana / no tokens** | You need full **`write-frontend-env-local.sh`** (not **`--urls-only`**) after Step 2 so **`VITE_*` bridge addresses** are set. Restart Vite. |
| **Bridge page: no tokens in the selector** | On the **server**, run **`make start-qa`** (includes **`qa:full-token-setup`**) or after **`make deploy`** run **`make qa-full-token-setup`**. Legacy: **`make deploy-tokens && make register-tokens`**. |
| **Settings → Faucet: “not deployed”** | That panel is for **optional faucet contracts** (separate from bridge test tokens). Bridge transfers use tokens registered by the full QA token setup. |
| **LocalTerra “Failed to fetch” or LCD shows `:1317`** | Regenerate **`.env.local`** after Step 2; confirm the script logs **`LCD=http://127.0.0.1:1318`** (shared QA remapping). Restart **`npm run dev`**. |
| **`localterra` exits (1), logs: `empty set` / `validator set` / replay error** | The **`localterra-data`** volume has **stale or incompatible chain state** (e.g. after an image upgrade). From the repo: **`docker compose down -v`** (removes named volumes), then **`make start-qa`**. Do not preserve that volume for LocalTerra QA unless you know it is valid. |
| **`localterra` container exits (1) (other)** | **`docker compose logs localterra`**. **Port in use**: **`E2E_TERRA_*`** in **`.env`** vs **`ss -tlnp`**. **ARM**: **`platform: linux/amd64`** needs QEMU or use **amd64**. |
| **`scp: open local ".deploy/local.env": No such file or directory`** | Your clone is missing **`.deploy/`** — **`git pull`** (the repo tracks **`.deploy/.gitkeep`**) or run **`mkdir -p .deploy`**. |
| **`scp` fails (other)** | Set **`QA_SSH_HOST`** / **`QA_SSH_PORT`** (e.g. in repo-root **`.env`**) and re-run **`make start-qa`** so the printed **`scp`** matches how you SSH. |
| **`ensure-terra-artifacts` / `cw20_mintable.wasm` fails** | **`scripts/ensure-cw20-mintable-wasm.sh`** tries **`scripts/download-cw20-wasm.sh`** first (wasmd-friendly release wasm), then **git** clone + **`cw20-base`** build (**`CW20_MINTABLE_RUSTFLAGS`**, optional **`wasm-opt --disable-bulk-memory`** if **binaryen** is installed). **`CW20_ENSURE_DOWNLOAD_FIRST=0`** forces git build first. If GitHub is blocked, set **`CW20_WASM_URL_OVERRIDE`** or copy **`cw20_mintable.wasm`** into **`artifacts/`**. |
| **Permission denied when writing Terra `artifacts/*.wasm`** | The **`packages/contracts-terraclassic/artifacts/`** tree is often root-owned if something was run with **sudo**. Run **`make start-qa`** as the same user that owns the repo, and fix ownership once: **`sudo chown -R "$(whoami):$(whoami)" packages/contracts-terraclassic/artifacts`**. Same idea if **`bridge.wasm`** fails to write. |
| **Terra deploy: `bulk memory support is not enabled` / Wasm deserialization** | Prefer the **release** path: **`rm -f packages/contracts-terraclassic/artifacts/cw20_mintable.wasm`**, **`git pull`**, **`make start-qa`** so **`ensure-cw20-mintable-wasm.sh`** fetches **cw-plus** release wasm first. For local builds, defaults use **`-C target-cpu=mvp`** and **`wasm-opt --disable-bulk-memory`** when **binaryen** is installed. Override with **`CW20_MINTABLE_RUSTFLAGS`** if needed. |

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
