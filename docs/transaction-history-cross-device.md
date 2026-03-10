# Transaction History: Cross-Device Limitation & Future On-Chain Query Approach

## Current State

Transaction history is stored **exclusively in browser `localStorage`** (key: `cl8y-bridge-transactions`, max 100 records). Records are created in `TransferForm.tsx` when a deposit is submitted and never synced to any backend or on-chain index.

**Consequence:** Transactions initiated on one device (e.g. PC) are invisible on another device (e.g. iPhone), even when connected with the same wallet. There is no cross-device persistence.

### Relevant code

| Component | File | Role |
|-----------|------|------|
| Transfer store | `stores/transfer.ts` | Reads/writes `localStorage`, `recordTransfer()`, `updateTransferRecord()` |
| Transfer types | `types/transfer.ts` | `TransferRecord` shape |
| History UI | `components/TransactionHistory.tsx` | Full history page, reads from localStorage |
| Recent transfers | `components/transfer/RecentTransfers.tsx` | Shows last N transfers on transfer page |
| Status refresh | `hooks/useTransferStatusRefresh.ts` | Polls non-terminal transfers to update lifecycle |

## Why On-Chain Queries Are Not Currently Feasible

Loading transaction history from RPC/LCD by wallet address is theoretically possible but blocked by infrastructure:

- **No historical/archive RPC node available.** Standard RPC providers have limited block history, and `eth_getLogs` lookback is capped (e.g. BSC publicnode caps at 50k blocks, ~1 day). Older deposits would be invisible.
- Some providers (e.g. `bsc-dataseed1`) don't support `eth_getLogs` at all.

## Future Approach: On-Chain Wallet History (if archive nodes become available)

If an archive RPC node or indexer becomes available, the following approach would work without any backend changes:

### Terra (Cosmos) — LCD tx search

The LCD has a built-in transaction search endpoint with event-based filtering:

```
GET /cosmos/tx/v1beta1/txs?events=message.sender='{terra_address}'&events=execute._contract_address='{bridge_address}'&order_by=ORDER_BY_DESC&pagination.limit=50
```

This returns all transactions where the wallet interacted with the bridge contract. The response includes full wasm event data (deposit amounts, nonces, dest chains, hashes). The existing `fetchLcd()` utility in `services/lcdClient.ts` can be reused — it's just a new path.

### EVM — getLogs with client-side filtering

The bridge `Deposit` event (`IBridge.sol`):

```solidity
event Deposit(
    bytes4 indexed destChain,
    bytes32 indexed destAccount,
    bytes32 srcAccount,      // NOT indexed
    address token,
    uint256 amount,
    uint64 nonce,
    uint256 fee
);
```

Since `srcAccount` is **not indexed**, filtering by depositor requires scanning all `Deposit` events from the bridge contract and filtering client-side. The `hashMonitor.ts` service (`fetchEvmDepositHashes()`) already implements this pattern — chunked `getLogs`, ABI decoding, and `xchainHashId` computation.

For incoming transfers, `destAccount` **is indexed** and can be used as a topic filter directly.

### Implementation sketch

A `fetchWalletBridgeHistory()` service would:

1. **Terra**: Call LCD tx search by sender + bridge contract address. Parse wasm events to extract deposit details.
2. **EVM**: Adapt `fetchEvmDepositHashes()` from `hashMonitor.ts` — scan `Deposit` events, filter where `srcAccount` matches the connected wallet (converted to bytes32 via `evmAddressToBytes32()`).
3. **Merge** on-chain results with existing localStorage records, dedup by `xchainHashId`, and persist new records to localStorage so the status refresh hooks continue working.

### Dependencies

- Archive RPC node for EVM chains (to query beyond the ~1 day `getLogs` window)
- Or a third-party indexer (e.g. The Graph, Covalent, custom subgraph)
- Terra LCD tx search works with standard LCD endpoints (no archive node needed) but may be slow for wallets with many transactions
