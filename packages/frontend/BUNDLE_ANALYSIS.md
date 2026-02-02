# Frontend Bundle Analysis

**Date:** 2026-02-02  
**Sprint:** 8 - Integration Validation & Production Hardening

## Summary

The initial page load is optimized at **~35KB (10KB gzipped)**, meeting the performance requirements. However, the Terra wallet chunk is **5.3MB (604KB gzipped)**, which is loaded when users connect a Terra wallet.

## Bundle Breakdown

| Chunk | Size | Gzipped | Description |
|-------|------|---------|-------------|
| `index.js` | 35 KB | 10 KB | Main entry point |
| `crypto.js` | 41 KB | 16 KB | Crypto utilities (secp256k1, noble) |
| `vendor-state.js` | 42 KB | 13 KB | State management (zustand, tanstack) |
| `vendor-react.js` | 142 KB | 46 KB | React core |
| `wallet-evm.js` | 173 KB | 51 KB | EVM wallet (wagmi, viem) |
| `walletconnect.js` | 297 KB | 91 KB | WalletConnect |
| **`wallet-terra.js`** | **5,292 KB** | **604 KB** | Terra wallet (cosmes) |

**Initial Load:** ~47KB gzipped (index + CSS)  
**Total with all wallets:** ~880KB gzipped

## Root Cause: Cosmes Protobufs

The `@goblinhunt/cosmes` package includes protobuf definitions for the entire Cosmos ecosystem:

```
protobufs/    57 MB   (All Cosmos proto definitions)
wallet/       1.1 MB  (Wallet implementations)
client/       792 KB  (LCD/RPC clients)
codec/        196 KB  (Encoding utilities)
registry/     128 KB  (Type registry)
typeutils/    20 KB   (Type utilities)
```

The protobufs are compiled into JavaScript and bundled together, even if only a small subset is used for Terra Classic.

## Mitigation Strategies Evaluated

### 1. Tree-shaking (Limited Effect)
- Vite/Rollup do tree-shake the package
- However, the protobuf types are interconnected via the type registry
- Using any Cosmos message pulls in the full protobuf dependency chain
- **Result:** Minimal size reduction possible

### 2. Alternative Libraries Evaluated

| Library | Size | Pros | Cons |
|---------|------|------|------|
| cosmos-kit | ~2MB | Official Cosmos ecosystem | Still includes protobufs, less Terra Classic support |
| terra.js (deprecated) | ~500KB | Smaller | No longer maintained, Terra 2.0 focused |
| Direct LCD calls | ~50KB | Smallest | No wallet integration, manual signing |

**Conclusion:** No drop-in replacement provides significantly smaller bundles while maintaining Terra Classic wallet support.

### 3. Lazy Loading (Currently Implemented)
- Terra wallet chunk is already lazy-loaded
- Only downloaded when user clicks "Connect Terra Wallet"
- Initial load unaffected (35KB gzipped)
- **This is the best current strategy**

### 4. Service Worker Caching (Recommended)
- Add service worker to cache wallet chunks
- First load: 604KB gzipped download
- Subsequent loads: Instant from cache
- Good for returning users

### 5. Code Splitting by Wallet Type (Future)
```typescript
// Instead of loading all Terra wallet connectors:
const StationConnector = lazy(() => import('./connectors/station'))
const LeapConnector = lazy(() => import('./connectors/leap'))
// Only load the connector user selects
```
- Would reduce initial Terra wallet load
- Requires UI changes to show wallet selection first

## Acceptance Criteria Status

| Criteria | Status | Notes |
|----------|--------|-------|
| Understand 5.3MB chunk cause | ✅ | Cosmes protobufs (57MB source) |
| Reduce to under 2MB | ❌ | Not feasible without library change |
| Document findings | ✅ | This document |
| Initial load under 150KB gzipped | ✅ | ~47KB gzipped |

## Recommendations

1. **Keep Current Implementation** - Lazy loading is effective; initial load is fast
2. **Add Service Worker** - Cache large chunks for returning users
3. **Monitor Cosmes** - Watch for future versions with smaller protobuf builds
4. **Consider cosmos-kit** - If needing broader Cosmos chain support in future
5. **Document for Users** - First-time Terra wallet connection may take a few seconds on slow connections

## Performance Expectations

| Connection | Initial Load | + Terra Wallet |
|------------|--------------|----------------|
| 4G (12 Mbps) | ~0.5s | +4s first time, cached after |
| 3G (1.5 Mbps) | ~2s | +30s first time, cached after |
| WiFi (50 Mbps) | ~0.1s | +1s first time, cached after |

The lazy-loading approach ensures the app loads quickly, with the Terra wallet overhead only affecting users who connect a Terra wallet.
