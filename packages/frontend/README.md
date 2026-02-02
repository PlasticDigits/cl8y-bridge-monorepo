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

### Features

- **Wallet Connection**: Connect EVM wallets (MetaMask, etc.) via wagmi
- **Bridge Form**: Transfer tokens between Terra Classic and EVM chains
- **Transaction History**: Track your bridge transactions (stored locally)

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
