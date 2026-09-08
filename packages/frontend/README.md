# CL8Y Bridge Frontend

Web application for the CL8Y Bridge interface.

## Tech Stack

- React 18 + TypeScript
- Vite for bundling
- TailwindCSS for styling
- wagmi + viem for EVM wallet connections
- @tanstack/react-query for data fetching

## Setup

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Build for production
npm run build

# Preview production build
npm run preview
```

## Development

The frontend runs on `http://localhost:3000` by default.

### Environment Variables

Create a `.env` file with:

```env
VITE_EVM_RPC_URL=http://localhost:8545
VITE_TERRA_LCD_URL=http://localhost:1317
VITE_EVM_BRIDGE_ADDRESS=0x...
VITE_TERRA_BRIDGE_ADDRESS=terra1...
```

Mainnet Solana: set `VITE_SOLANA_PROGRAM_ID`, `VITE_SOLANA_RPC_URL`, and noneconomic test SPL mints (`VITE_SOLANA_TESTA_MINT`, `VITE_SOLANA_TESTB_MINT`, `VITE_SOLANA_TDEC_MINT`) per the monorepo [README.md](../../README.md) (Solana mainnet-beta). The **BridgeConfig PDA** for mainnet is `HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD` (documented next to the program id there and in [docs/deployment-solana-mainnet.md](../../docs/deployment-solana-mainnet.md)); the app derives it from the program id. See `.env.example` for commented defaults.

### Features

- **Wallet Connection**: Connect EVM wallets (MetaMask, etc.) via wagmi
- **Bridge Form**: Transfer tokens between Terra Classic and EVM chains
- **Transaction History**: Track your bridge transactions (stored locally)

### Dependency patches (`patch-package`)

The app ships a [patch for `rpc-websockets`](./patches/rpc-websockets+9.3.3.patch) (a dependency of `@solana/web3.js`). Without it, some Solana JSON-RPC WebSocket responses that include both `error: null` and `result` are misclassified as malformed, which breaks `confirmTransaction` / `signatureSubscribe` in the browser (see issue #106). Regression coverage lives in `src/services/solana/jsonRpcWebsocketResponse.test.ts`.

### Solana as source chain

When bridging **from Solana**, the app loads `TokenMapping.local_mint` for the route. If it is the **wrapped SOL** mint, the transaction uses **`deposit_native`** (lamports). For any other SPL mint, it uses **`deposit_spl`** from the user’s ATA (and may create the ATA first). See [docs/SOLANA_BRIDGE_DEPOSITS.md](../../docs/SOLANA_BRIDGE_DEPOSITS.md).

### CL8Y Legal clickwrap (GL-134)

After a source (or destination) wallet is connected, mutative deposit / hash-submit / Solana execute controls require **`signed_latest`** for property **`bridge.cl8y.com`** via `@plasticdigits/cl8y-clickwrap`. Unsigned users are sent to [terms.cl8y.com](https://terms.cl8y.com); the header Connect buttons stay usable. Invariant: [INV-FE-CLICKWRAP-1](../../docs/FRONTEND_BRIDGE_INVARIANTS.md). Skill: [`skills/agent-frontend-clickwrap.md`](../../skills/agent-frontend-clickwrap.md).

The SDK is published to GitLab npm (project `82547916`), not npmjs. Pin **exact** `@plasticdigits/cl8y-clickwrap@0.1.1`. `packages/frontend/.npmrc` sets `@plasticdigits:registry`. Local status checks need Legal **`CORS_ORIGINS`** to include `http://localhost:3000`; portal return needs **`VITE_REDIRECT_URI_ALLOWLIST`** or **`VITE_ALLOW_LOCALHOST_REDIRECT`**. Optional public overrides: `VITE_CLICKWRAP_API_BASE_URL`, `VITE_CLICKWRAP_TERMS_BASE_URL` (production accepts only `https://api.terms.cl8y.com` / `https://terms.cl8y.com`; ignores `http:` and other hosts).

## Project Structure

```
src/
├── components/
│   ├── ConnectWallet.tsx   # EVM wallet connection
│   ├── BridgeForm.tsx      # Main bridge interface
│   └── TransactionHistory.tsx
├── lib/
│   ├── wagmi.ts            # wagmi configuration
│   └── chains.ts           # Chain definitions
├── App.tsx                 # Main app component
├── main.tsx               # React entry point
└── index.css              # Tailwind styles
```

## TODO (Sprint 5+)

- [ ] Terra wallet connection (@classic-terra/wallet-kit)
- [ ] Actual bridge transaction submission
- [ ] Real-time transaction status from relayer API
- [ ] Token selection with balances
- [ ] Gas estimation
- [ ] Mobile responsive improvements
