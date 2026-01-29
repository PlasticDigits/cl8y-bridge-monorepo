<div style="text-align: center; padding-top: 200px;">

# CL8Y Ecosystem Whitepaper

<br><br>

## Building the Future of Cross-Chain Infrastructure and Decentralized Finance

<br><br><br>

**Cross-Chain Bridge • DeFi • GameFi**

<br><br>

*Secure. Transparent. Decentralized.*

<br><br><br><br>

**Version 3.0**

</div>

<div style="page-break-after: always;"></div>

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Introduction](#introduction)
3. [CL8Y Bridge](#cl8y-bridge)
4. [Progressive Decentralization](#progressive-decentralization)
5. [USTR CMM: The Collateralized Unstablecoin System](#ustr-cmm-the-collateralized-unstablecoin-system)
6. [DeFi Ecosystem](#defi-ecosystem)
7. [GameFi Platform: PROTOCASS](#gamefi-platform-protocass)
8. [Technical Architecture](#technical-architecture)
9. [Security Framework](#security-framework)
10. [Governance](#governance)
11. [Roadmap](#roadmap)
12. [Conclusion](#conclusion)

<div style="page-break-after: always;"></div>

## Executive Summary

CL8Y is a comprehensive blockchain ecosystem designed to provide secure cross-chain infrastructure, innovative decentralized finance (DeFi) products, and engaging GameFi experiences. At its core, the CL8Y Bridge enables seamless asset transfers between Ethereum Virtual Machine (EVM) compatible chains and Terra Classic, while the broader ecosystem introduces UST1—a novel collateralized unstablecoin—alongside a suite of DeFi applications including a next-generation DEX, perpetual futures exchange, and oracle-free lending protocol.

**Key Highlights:**

- **Cross-Chain Bridge**: Secure, battle-tested bridge infrastructure connecting EVM chains with Terra Classic
- **Progressive Decentralization**: Planned evolution from centralized operator to DAO-governed validation
- **UST1 Unstablecoin**: Over-collateralized stablecoin mechanism with dynamic minting based on collateralization ratios
- **DeFi Suite**: Comprehensive trading, lending, and liquidity provision with innovative UST1 burn economics
- **GameFi Platform**: Skill-based text RPG with verifiable gameplay and LLM-generated content
- **Hyperlane Compatibility**: Future-proofed architecture for permissionless interchain messaging

---

## Introduction

### The Challenge

The blockchain industry faces several interconnected challenges:

1. **Fragmented Liquidity**: Assets are siloed across incompatible chains, limiting capital efficiency
2. **Bridge Security**: Cross-chain bridges remain one of the most exploited attack vectors in DeFi
3. **Trust Requirements**: Most bridges rely on centralized operators or small multisig sets
4. **Terra Classic Recovery**: The ecosystem requires robust, transparent financial infrastructure following the USTC depeg event

### The CL8Y Solution

CL8Y addresses these challenges through a vertically integrated approach:

- **Secure Bridge Infrastructure**: Multi-layered security with watchtower monitoring, timelock delays, and progressive decentralization
- **Modular Validation**: Architecture designed to evolve from centralized operation to fully decentralized validation
- **Transparent Collateralization**: UST1's over-collateralized model with clear, auditable reserves
- **Burn Economics**: Novel fee mechanisms that reduce token supply while incentivizing ecosystem participation

---

## CL8Y Bridge

### Overview

The CL8Y Bridge is the foundational infrastructure enabling cross-chain asset transfers between EVM-compatible blockchains (Ethereum, BSC, Polygon, etc.) and Terra Classic. The bridge supports native token wrapping, allowing users to access liquidity across ecosystems while maintaining security guarantees.

### Design Philosophy: Accountable Speed

CL8Y Bridge makes an unconventional design choice: **a single centralized operator for transaction approval, secured by a decentralized network of cancelers**. This hybrid approach delivers the best of both worlds:

| Aspect | Traditional Multisig | CL8Y Approach |
|--------|---------------------|---------------|
| **Speed** | Slow (wait for N signatures) | Fast (single operator) |
| **Cost** | High (multiple signers pay gas) | Low (one approval tx) |
| **Security** | Pre-execution consensus | Post-approval monitoring |
| **Accountability** | Diffused across signers | Clear operator responsibility |

**How it works:**

1. **Operator Approves**: A single operator submits withdrawal approvals quickly and efficiently
2. **Delay Window**: All approvals enter a mandatory waiting period (default: 5 minutes)
3. **Canceler Network Monitors**: Independent canceler nodes verify each approval against source chain
4. **Execute or Cancel**: Valid transfers execute after delay; fraudulent ones are cancelled

This model inverts the traditional security paradigm: instead of requiring consensus *before* action, CL8Y enables fast action with the ability to *undo* malicious behavior during the delay window.

### Canceler Network: Decentralized Security on opBNB

The canceler network provides security without sacrificing speed:

<div style="page-break-inside: avoid;">

```
┌─────────────────────────────────────────────────────────────┐
│                   Canceler Network                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ┌─────────────┐                                           │
│   │  Operator   │── Approve ──►┌──────────────┐             │
│   │  (Single)   │              │              │             │
│   └─────────────┘              │    Delay     │             │
│                                │    Window    │             │
│   ┌─────────────┐              │   (5 min)    │             │
│   │ Canceler 1  │── Monitor ──►│              │──► Execute  │
│   │ (opBNB)     │              │              │     OR      │
│   └─────────────┘              │              │   Cancel    │
│   ┌─────────────┐              │              │             │
│   │ Canceler 2  │── Monitor ──►│              │             │
│   │ (opBNB)     │              └──────────────┘             │
│   └─────────────┘                                           │
│   ┌─────────────┐                                           │
│   │ Canceler N  │── Monitor ──►      ...                    │
│   │ (Raspberry Pi)                                          │
│   └─────────────┘                                           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

</div>

**Key Properties:**

- **Cheap to Run**: Cancelers operate on **opBNB** (BNB Chain's L2), where gas costs are fractions of a cent
- **Low Hardware**: A **Raspberry Pi** is sufficient to run a canceler node—no expensive infrastructure required
- **Asymmetric Economics**: Approving costs the operator gas; cancelling is cheap for anyone watching
- **One Honest Canceler Wins**: Only one canceler needs to catch and cancel a fraudulent approval
- **Permissionless Monitoring**: Anyone can run a canceler and earn reputation for protecting the bridge

### Core Architecture

<div style="page-break-inside: avoid;">

```
┌────────────────────────────────────────────────────────────┐
│                        CL8Y Bridge                         │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  ┌─────────────┐                         ┌─────────────┐   │
│  │   EVM       │                         │   Terra     │   │
│  │   Chains    │◄───────────────────────►│   Classic   │   │
│  └─────────────┘                         └─────────────┘   │
│        │                                       │           │
│        ▼                                       ▼           │
│  ┌─────────────┐                         ┌─────────────┐   │
│  │   Bridge    │                         │   CosmWasm  │   │
│  │   Contract  │                         │   Contract  │   │
│  │   (EVM)     │                         │   (Terra)   │   │
│  └─────────────┘                         └─────────────┘   │
│        │                                       │           │
│        └───────────────┬───────────────────────┘           │
│                        │                                   │
│                        ▼                                   │
│                ┌───────────────┐                           │
│                │    Relayer    │                           │
│                │    Network    │                           │
│                └───────────────┘                           │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

</div>

### Supported Chains

- **EVM Chains**: Ethereum, BNB Smart Chain (BSC), Polygon, Arbitrum, Optimism
- **Cosmos Chains**: Terra Classic (LUNC/USTC)

### Key Features

1. **Wrapped Token Minting**: Bridge-wrapped tokens (xxx-cb suffix) maintain 1:1 backing with source assets
2. **Bi-directional Transfers**: Seamless deposits and withdrawals in both directions
3. **Canonical TransferId**: Cryptographic hashing ensures transfer uniqueness and prevents replay attacks
4. **Watchtower Security**: Delay windows enable canceler monitoring before fund release
5. **opBNB Cancelers**: Ultra-low-cost security monitoring on BNB Chain's L2
6. **Raspberry Pi Nodes**: Canceler nodes run on minimal hardware, enabling broad participation

---

## Progressive Decentralization

### Philosophy

CL8Y follows a "Sovereignty First" principle: the bridge must never depend on external infrastructure for core functionality. Decentralization is achieved progressively, ensuring security at each stage while expanding the validator set.

### Current State: Centralized Operator

The initial deployment operates with a centralized operator model:

- **Fast Execution**: Single-signature authorization for efficient processing
- **Clear Accountability**: Operator identity known and accountable
- **Rapid Response**: Ability to pause operations during security incidents

### Evolution Path

#### Stage 1: Multisig Validation

```
Operator → 3-of-5 Multisig → 5-of-9 Multisig
```

- Increase signer set progressively
- Implement threshold signature schemes
- Geographic and organizational distribution of signers

#### Stage 2: Modular Validation Framework

The bridge architecture supports swappable validation modules through the `IValidationModule` interface:

```solidity
interface IValidationModule {
    function validateTransfer(
        bytes32 transferId,
        uint256 amount,
        address token,
        address recipient,
        bytes calldata proof
    ) external view returns (bool);
    
    function getModuleType() external pure returns (string memory);
}
```

**Available Modules:**

| Module | Description | Trust Model |
|--------|-------------|-------------|
| OperatorValidationModule | Single authorized signer | Centralized |
| MultisigValidationModule | M-of-N threshold signatures | Federated |
| HyperlaneValidationModule | Hyperlane ISM integration | Permissionless |

#### Stage 3: Hyperlane Integration

Future compatibility with Hyperlane's Interchain Security Modules (ISMs) enables:

- **Permissionless Validation**: Anyone can verify cross-chain messages
- **Economic Security**: Staked validators with slashing conditions
- **Modular Security**: Composable security models per route

**Token Swap Architecture:**

For tokens where liquidity exists on both CL8Y Bridge and Hyperlane:

<div style="page-break-inside: avoid;">

```
┌─────────────────────────────────────────────────────────────┐
│                    Token Swap System                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   USDC-cb ──┐                             ┌── hypUSDC       │
│   WETH-cb ──┼── SwapRouter (1:1 swap) ────┼── hypWETH       │
│   WBTC-cb ──┘                             └── hypWBTC       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

</div>

---

## USTR CMM: The Collateralized Unstablecoin System

### Overview

The USTR Collateralized Monetary Market (CMM) introduces a transparent, over-collateralized stablecoin system designed specifically for the Terra Classic ecosystem. The system consists of two primary tokens:

- **USTR (Repeg Token)**: Utility token acquired through USTC deposits (not a governance token)
- **UST1 (Unstablecoin)**: Over-collateralized stablecoin backed by USTC and other assets
- **CL8Y**: The sole governance token for DAO voting and node staking

### USTR Token Economics

#### Acquisition Mechanism

USTR is acquired by depositing USTC into the Treasury contract. The swap rate increases over time to incentivize early adoption:

```
Launch Rate:  1.5 USTC per 1 USTR (best rate for early adopters)
Final Rate:   2.5 USTC per 1 USTR (after 100 days)
Duration:     Linear increase over 100 days
```

Early participants benefit from the lower USTC cost per USTR, rewarding those who commit to the ecosystem first.

#### Referral System

Users can register referral codes by burning USTR:

- **Registration Cost**: Configurable USTR burn amount
- **Swapper Bonus**: Additional USTR for users using referral codes
- **Referrer Bonus**: USTR rewards for code owners based on referred swap volume

### UST1 Collateralization

UST1 minting and redemption is governed by the Treasury's Collateralization Ratio (CR):

| CR Tier | Ratio Range | Minting | Redemption |
|---------|-------------|---------|------------|
| **BLUE** | ≥ 200% | Full capacity | Full capacity |
| **GREEN** | 150-200% | Reduced | Full |
| **YELLOW** | 120-150% | Minimal | Partial |
| **RED** | < 120% | Disabled | Emergency only |

### 5-Year Rolling Yield Pools

Treasury yields are distributed through a structured pool system designed for long-term sustainability. Collateral generates yield which is allocated across four distinct pools on a rolling 5-year schedule:

<div style="page-break-inside: avoid;">

```
┌─────────────────────────────────────────────────────────────┐
│                  5-Year Rolling Pools                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Treasury Yield ──┬──► Pool A: USTR Buy & Burn             │
│                    │    (Deflationary pressure on USTR)     │
│                    │                                        │
│                    ├──► Pool B: UST1 Yield                  │
│                    │    (Rewards for UST1 holders)          │
│                    │                                        │
│                    ├──► Pool C: USTR Yield                  │
│                    │    (Rewards for USTR stakers)          │
│                    │                                        │
│                    └──► Pool D: Treasury DAO                │
│                         (Controlled by CL8Y node operators) │
│                                                             │
│   ◄─────────── 5-Year Rolling Distribution ───────────►     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

</div>

| Pool | Purpose | Governance |
|------|---------|------------|
| **Pool A: USTR Buy & Burn** | Market purchases of USTR followed by permanent burn, creating deflationary tokenomics | Automatic, protocol-controlled |
| **Pool B: UST1 Yield** | Distributed to UST1 holders proportionally, incentivizing stablecoin adoption | Automatic, protocol-controlled |
| **Pool C: USTR Yield** | Distributed to USTR stakers, rewarding long-term ecosystem participants | Automatic, protocol-controlled |
| **Pool D: Treasury DAO** | Discretionary funds for ecosystem development, grants, and strategic initiatives | CL8Y node operators via DAO governance |

**Rolling Mechanism:**
- Yields are calculated and allocated on a continuous basis
- 5-year rolling window smooths volatility and ensures sustainable distributions
- Pool allocations can be adjusted via CL8Y node governance (requires Tier 3 Treasury proposal)

### Treasury Structure

<div style="page-break-inside: avoid;">

```
┌─────────────────────────────────────────────────────────────┐
│                      UST1 Treasury                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ┌─────────────────┐    ┌─────────────────┐                │
│   │  USTC Deposits  │    │  Other Assets   │                │
│   │    (Primary)    │    │   (Secondary)   │                │
│   └────────┬────────┘    └────────┬────────┘                │
│            │                      │                         │
│            └──────────┬───────────┘                         │
│                       ▼                                     │
│            ┌─────────────────────┐                          │
│            │   Collateral Pool   │                          │
│            │   (Auditable)       │                          │
│            └──────────┬──────────┘                          │
│                       │                                     │
│                       ▼                                     │
│            ┌─────────────────────┐                          │
│            │   CR Calculation    │──► Tier Assignment       │
│            └─────────────────────┘                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

</div>

---

## DeFi Ecosystem

### UST1 Burn DEX

A next-generation decentralized exchange featuring innovative economics designed to maximize UST1 burn:

#### Pool Types

1. **V2 Pools (Constant Product AMM)**
   - Traditional x*y=k model
   - Simple liquidity provision
   - Suitable for long-tail assets

2. **V3 Pools (Concentrated Liquidity)**
   - Capital-efficient liquidity within price ranges
   - Higher yields for active LPs
   - Optimal for stable pairs

#### Revolutionary Fee Model

Unlike traditional DEXs where fees go to liquidity providers, the CL8Y DEX burns all fees as UST1:

| Fee Type | Rate | Destination |
|----------|------|-------------|
| Flat Trade Fee | Variable | 100% UST1 Burn |
| Exit Fee | 2.99% | 100% UST1 Burn |
| Pool Swap Fee | 0.3% | 100% UST1 Burn |

#### Advanced Trading Wallet

Frequent traders can deposit assets into an on-chain custodial wallet to avoid per-trade exit fees after a one-time UST1 burn.

#### Discount Tier System

Progressive fee reductions based on cumulative UST1 burns:

| Tier | Cumulative Burns | Fee Discount |
|------|-----------------|--------------|
| Bronze | 100 UST1 | 10% |
| Silver | 1,000 UST1 | 20% |
| Gold | 10,000 UST1 | 35% |
| Platinum | 100,000 UST1 | 50% |

### Perpetual Futures DEX

A fully on-chain perpetual futures exchange exclusively supporting CW20 tokens with UST1 as collateral:

#### Toxic Flow Protection

Innovative mechanisms to combat MEV and front-running:

1. **Batch Auctions**: Orders collected and executed in discrete batches
2. **Commit-Reveal**: Two-phase order submission prevents front-running
3. **Dynamic Spreads**: Spread widens based on recent volatility and toxic flow metrics
4. **Rate Limiting**: Position size limits per time window

#### Risk Management

- **Isolated & Cross Margin**: Flexible margin modes per position or account
- **Auto-Deleveraging (ADL)**: Automatic position reduction during extreme market conditions
- **Insurance Fund**: Protocol-owned fund covers liquidation shortfalls
- **Leverage Tiers**: Progressive margin requirements based on position size

### Oracle-Free Lending Protocol

An innovative money market inspired by Ajna, utilizing market-driven price discovery:

#### Core Innovation: Smart Bucket Aggregation

Instead of external oracles, lenders specify price buckets where they're willing to lend:

<div style="page-break-inside: avoid;">

```
┌─────────────────────────────────────────────────────────────┐
│                   Price Bucket System                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Bucket 1: $0.95-$1.00  │  500,000 UST1 deposited          │
│   Bucket 2: $0.90-$0.95  │  300,000 UST1 deposited          │
│   Bucket 3: $0.85-$0.90  │  200,000 UST1 deposited          │
│                                                             │
│   Aggregated Liquidity: Automatic consolidation for         │
│   borrower-friendly UX                                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

</div>

#### Key Features

- **No Oracle Dependency**: Price discovery through lender behavior
- **Risk Tier Abstraction**: Simplified UX with Conservative/Moderate/Aggressive presets
- **Adaptive Liquidation Auctions**: Exponential decay auctions for efficient liquidation
- **Composable Vault Layer**: Yield aggregation and strategy vaults

### CMM Marketplace

#### Cross-Chain FDUSD Swaps

Seamless swaps between wrapped FDUSD (wFDUSD) and ecosystem tokens:

- **Liquidity Source**: Venus-staked FDUSD on BSC
- **Symmetric Fees**: 3.5% fee on both buy and sell sides
- **Bridge Integration**: Automated cross-chain settlement

#### Gamified English Auctions

Oracle-free price discovery for non-oracle assets:

- **Anti-Snipe Mechanism**: Bids in final minutes extend auction
- **Achievement System**: Rewards for participation milestones
- **Streak Bonuses**: Consecutive bidding rewards
- **Prize Pools**: Community-funded rewards for active participants

---

## GameFi Platform: PROTOCASS

### Vision

PROTOCASS is a skill-based text RPG platform that prioritizes verifiable, deterministic gameplay over RNG-dependent mechanics. The platform leverages Web 2.5 architecture to balance scalability with blockchain verification.

### Architecture

<div style="page-break-inside: avoid;">

```
┌─────────────────────────────────────────────────────────────┐
│                     PROTOCASS Architecture                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│   │   Frontend  │    │   ImmuneDB  │    │  CosmWasm   │     │
│   │   (React)   │◄──►│   (Game DB) │◄──►│   Vaults    │     │
│   └─────────────┘    └─────────────┘    └─────────────┘     │
│                             │                               │
│                             ▼                               │
│                    ┌─────────────────┐                      │
│                    │  On-Chain Root  │                      │
│                    │    Anchoring    │                      │
│                    └─────────────────┘                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

</div>

### Core Features

#### Skill-Based Gameplay

- **No RNG**: All outcomes determined by player choices and skill
- **Deterministic Logic**: Given same inputs, always same outputs
- **Verifiable Replays**: Any game session can be independently verified

#### LLM-Native Content

- **Procedural Generation**: AI-generated quests, dialogues, and narratives
- **Creator Economy**: Players can create and monetize content
- **Dynamic World**: Content evolves based on player actions

#### ImmuneDB

A specialized database providing:

- **Immutable Records**: Game state cannot be retroactively altered
- **Cryptographic Verification**: Merkle proofs for any state query
- **Atomic Transactions**: Complex game operations execute completely or not at all
- **Periodic Anchoring**: Root hashes committed to blockchain

#### UST1 Burn Economics

All platform fees are burned as UST1:

- **Entry Fees**: Tournament and competition entry
- **Marketplace Fees**: Asset trading commissions
- **Premium Features**: Advanced gameplay options

### Flagship Title: Karnyx — Solar Tigers Onchain

Karnyx is the first flagship game built on PROTOCASS, showcasing the platform's capabilities through an immersive AI-narrative experience.

#### The Living Ledger

The world of Karnyx is vast, dangerous, and decentralized. Players become **Solar Tigers**—cybernetic hunters forging empires across hostile biomes and sentient villages guarding ancient assets. Explore, hunt, negotiate, or conquer to expand influence; every action is permanently recorded onchain.

- **Neon Jungles & Mythic Futurism**: Relic shrines, aqua-lit outposts, and violet swamps form a frontier of mythic futurism
- **Immutable Actions**: Every strategic decision ripples through clans and markets as permanent ledger entries
- **Real-Time Politics**: From jungle ambushes to war councils, choices shape persistent politics and evolving lore

#### TigerHunt: Temples, Power & Prestige

In TigerHuntV2, Tigers commission monumental temples forged from rare materials. The game economy honors two distinct paths:

| Path | Focus | Progression |
|------|-------|-------------|
| **Power** | Combat, conquest, territory control | Military dominance |
| **Prestige** | Rituals, alliances, cultural influence | Social standing |

**Triangular Economy**: Wealth, effort, and devotion form a dynamic triangle—each leg advancing different axes of Power and Prestige. Tigers choose their vector and the world responds through markets, allegiances, and territory control.

#### Onchain Hunt Logs

Each Tiger maintains a daily hunt log stored onchain using byte packing for ultra-efficient storage:

<div style="page-break-inside: avoid;">

```
┌─────────────────────────────────────────────────────────────┐
│                    Hunt Log System                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Battles ──┐                                               │
│   Discoveries ─┼── Byte Packed ── Onchain Identity Link     │
│   Deals ────┘                                               │
│                         │                                   │
│                         ▼                                   │
│              ┌─────────────────────┐                        │
│              │   LLM Narrative     │                        │
│              │   Generation        │                        │
│              └─────────────────────┘                        │
│                         │                                   │
│                         ▼                                   │
│              Personalized Stories & Clan Chronicles         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

</div>

Language models read these compressed logs to generate personalized stories and evolving clan chronicles—creating unique narratives for every player.

#### Relics of the Terraformers

Ancient artifacts pulse beneath violet swamps and jungle roots, offering power at great risk:

- **Gravity Warpers**: Bend physics within territory boundaries
- **Whispering Tongues**: Artifacts speaking in lost languages, revealing hidden lore
- **Living Relics**: Sentient artifacts that form bonds with their wielders

*Power tempts—but awakening relics draws rivals and older forces.*

#### Lore Fragments

> *"The Glass Spine valley rings at dusk; Tigers claim its resonance for far-sight councils."*

> *"Lion's Gate monoliths bleed amber light when offered bone-keys carved from leviathan ribs."*

> *"Astra Looms stitch fate-threads between clans; cut one, and storms remember your name."*

#### Visual Identity

Karnyx embodies a luxury sci-fantasy aesthetic:

- **Foundation**: Deep black backgrounds with gold headline glow
- **Accents**: Selective aqua focus with subtle inner halos
- **Polish**: Gradient frames, soft glows, and screen-blended halos
- **Motion**: Tasteful, minimal animations respecting typographic hierarchy

### Featured Title: Merc Mania — Strategic Mercenary Mining Operations

Merc Mania brings tactical resource warfare to PROTOCASS, where mercenary companies compete for control of valuable extraction sites in contested territories.

#### The Theater of Operations

In the vast expanses of a resource-rich continent, powerful extraction sites lie scattered across contested territory. Multiple mercenary companies deploy their forces to secure lucrative mining operations, establishing temporary control through superior firepower and strategic positioning.

Every mine tells a story of shifting allegiances, tactical victories, and the relentless pursuit of mineral wealth that drives the modern mercenary economy.

#### Operational Systems

<div style="page-break-inside: avoid;">

```
┌─────────────────────────────────────────────────────────────┐
│                    Merc Mania Architecture                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Resource Command & Control                                │
│   ├── GameMaster: Secure escrow for company assets          │
│   ├── ResourceManager: Strategic material tracking          │
│   └── Mine: Capturable sites with decay mechanics           │
│                                                             │
│   Force Deployment                                          │
│   ├── MercRecruiter: Resource-to-military conversion        │
│   ├── MercAssetFactory: Unit creation & classification      │
│   └── GameAssetFactory: Token generation & distribution     │
│                                                             │
│   Combat Resolution                                         │
│   ├── Territorial Seizure: Assault mining facilities        │
│   ├── Defense Systems: Fortify & repel hostiles             │
│   └── Battle Calculator: Power based on level × quantity    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

</div>

#### Mission Objectives

**Primary Operations:**

- **Secure Mining Rights**: Deploy mercenary forces to capture resource extraction facilities
- **Resource Acquisition**: Extract valuable materials from controlled territories
- **Force Multiplication**: Recruit and upgrade units using strategic resource combinations
- **Economic Warfare**: Disrupt competitor operations while defending your own assets

**Tactical Considerations:**

- **Resource Diversification**: Higher-level mercenaries require multiple material types
- **Operational Security**: Fortify positions using Gold-based defense systems
- **Asset Recovery**: Strategic abandonment preserves 90% of deployed forces
- **Supply Chain Management**: All operations require Gold as base currency

#### Strategic Framework

**Economic Principles:**

| Mechanism | Effect |
|-----------|--------|
| Operational Costs | 50% asset burn on withdrawals maintains stability |
| Production Decay | Mining output halves every 72 hours |
| Combat Losses | Failed acquisitions result in complete unit loss |
| Defense Investment | Gold expenditure provides temporary advantages |

**Force Structure:**

| Level | Classification | Resource Requirements |
|-------|----------------|----------------------|
| 1 | Local Militia | Gold only |
| 2 | Professional Forces | Gold + 1 strategic material |
| 3 | Elite Operations | Gold + 2 strategic materials |
| 4 | Special Command | Gold + 3 strategic materials |
| 5 | Legendary Assets | Gold + 4 strategic materials |

#### Onchain Asset Management

All game assets are managed through smart contracts:

- **Escrow Security**: GameMaster contract holds all company assets with built-in operational costs
- **Verifiable Battles**: Combat outcomes determined by transparent, deterministic logic
- **Permanent Records**: Every territorial seizure and defense action recorded on-chain
- **UST1 Integration**: All transaction fees contribute to UST1 burn economics

---

## Technical Architecture

### Smart Contracts

#### EVM Contracts (Solidity)

| Contract | Purpose |
|----------|---------|
| CL8YBridgeV2 | Main bridge logic with modular validation |
| TokenFactory | Wrapped token deployment |
| SwapRouter | Token swap coordination |
| NodeStaking | DAO validator staking |
| NodeGovernor | Governance proposal management |
| DAOTimelockController | Execution delay enforcement |

#### CosmWasm Contracts (Rust)

| Contract | Purpose |
|----------|---------|
| cl8y_bridge | Terra Classic bridge endpoint |
| treasury | UST1 collateral management |
| swap | USTC-to-USTR conversion |
| referral | Referral code registry |
| dex_router | Omnirouter for swap aggregation |
| perp_engine | Perpetual futures matching |
| money_market | Lending pool logic |

### Backend Services

| Service | Technology | Purpose |
|---------|------------|---------|
| Relayer | Rust | Cross-chain message relay |
| Indexer | Node.js/PostgreSQL | Historical data aggregation |
| Oracle Aggregator | Rust | Multi-source price feeds |
| Keeper Network | Rust | Liquidation and maintenance |

### Frontend Applications

- **Bridge UI**: React/Next.js application for bridge operations
- **DEX Interface**: Trading terminal with TradingView integration
- **Governance Portal**: Proposal viewing and voting interface
- **PROTOCASS Client**: Game client with real-time updates

---

## Security Framework

### Multi-Layer Security Model

<div style="page-break-inside: avoid;">

```
┌─────────────────────────────────────────────────────────────┐
│                    Security Layers                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Layer 1: Smart Contract Security                          │
│   ├── Formal verification                                   │
│   ├── Multiple independent audits                           │
│   └── Bug bounty program                                    │
│                                                             │
│   Layer 2: Operational Security                             │
│   ├── Timelock delays on sensitive operations               │
│   ├── Watchtower monitoring with cancel capability          │
│   └── Rate limiting on large transfers                      │
│                                                             │
│   Layer 3: Economic Security                                │
│   ├── Slashing conditions for malicious validators          │
│   ├── Insurance fund for exploit recovery                   │
│   └── Progressive bonding requirements                      │
│                                                             │
│   Layer 4: Governance Security                              │
│   ├── Tiered proposal system                                │
│   ├── Supermajority requirements for critical changes       │
│   └── Emergency pause mechanisms                            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

</div>

### Watchtower Pattern

The watchtower security model provides a safety net for cross-chain operations:

1. **Approve**: Operator signals intent to release funds
2. **Delay**: Configurable waiting period (e.g., 1-24 hours)
3. **Monitor**: Watchtowers verify approval validity
4. **Execute or Cancel**: Legitimate transfers execute; suspicious ones are cancelled

<div style="page-break-inside: avoid;">

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│  Approve │───►│   Delay  │───►│  Monitor │───►│ Execute/ │
│          │    │  Window  │    │          │    │  Cancel  │
└──────────┘    └──────────┘    └──────────┘    └──────────┘
```

</div>

### Canonical TransferId

Every cross-chain transfer is uniquely identified:

```rust
fn compute_transfer_id(
    source_chain: &str,
    dest_chain: &str,
    sender: &str,
    recipient: &str,
    token: &str,
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    keccak256(encode([
        source_chain,
        dest_chain,
        sender,
        recipient,
        token,
        amount,
        nonce,
    ]))
}
```

This ensures:
- **Uniqueness**: No two transfers share the same ID
- **Verifiability**: ID can be independently computed on any chain
- **Replay Protection**: Used nonces are tracked and rejected

---

## Governance

### DAO Structure

CL8Y governance follows a progressive decentralization path:

```
Phase 1: Admin (Single Key)
    ↓
Phase 2: Multisig (3-of-5 → 5-of-9)
    ↓
Phase 3: Full DAO (Node-based Voting)
```

### Node-Based Governance

#### Staking Requirements

- **Stake Amount**: 100 CL8Y minimum per node
- **Hardware**: Raspberry Pi compatible (low barrier)
- **Unstake Cooldown**: 7 days to prevent vote manipulation

#### Voting Power

- **1 Node = 1 Vote**: Prevents plutocratic control
- **Active Participation Required**: Nodes must vote regularly to maintain standing

### Proposal Tiers

| Tier | Type | Quorum | Threshold | Timelock |
|------|------|--------|-----------|----------|
| 1 | Bridge Operations | 20% | 51% | 24 hours |
| 2 | Configuration | 30% | 60% | 48 hours |
| 3 | Treasury | 40% | 67% | 72 hours |
| 4 | Emergency | 10% | 75% | 6 hours |
| 5 | Slashing | 50% | 80% | 7 days |

### Optimistic Governance

For routine operations, an optimistic model reduces governance overhead:

- **Proposal Submission**: Guardian submits routine operation
- **Veto Window**: Time period for objections
- **Automatic Execution**: If no veto, operation proceeds
- **Veto Execution**: Single canceler can halt suspicious operations

---

## Roadmap

### Phase 1: Foundation

**Objective**: Establish core infrastructure and prove security model

- Deploy CL8Y Bridge on mainnet (EVM + Terra Classic)
- Implement watchtower pattern on both chains
- Launch canonical TransferId system
- Complete initial security audits
- Establish multisig governance (3-of-5)

### Phase 2: USTR CMM Launch

**Objective**: Launch collateralized unstablecoin system

- Deploy Treasury contract with CR tier logic
- Launch USTC-to-USTR swap mechanism
- Implement referral system
- Begin USTR distribution
- Conduct UST1 minting beta

### Phase 3: DEX Ecosystem

**Objective**: Build comprehensive trading infrastructure

- Launch V2 AMM pools
- Implement UST1 burn fee model
- Deploy Advanced Trading Wallet
- Launch V3 concentrated liquidity pools
- Integrate Omnirouter for optimal routing

### Phase 4: Advanced DeFi

**Objective**: Expand DeFi capabilities

- Deploy perpetual futures engine
- Implement batch auction system
- Launch oracle-free lending protocol
- Deploy CMM marketplace with FDUSD swaps
- Integrate gamified auction system

### Phase 5: Progressive Decentralization

**Objective**: Transition to full DAO governance

- Deploy node staking contracts
- Launch NodeGovernor with tiered proposals
- Expand multisig (5-of-9)
- Implement slashing mechanism
- Transition to optimistic governance for routine ops

### Phase 6: Hyperlane Integration

**Objective**: Enable permissionless validation

- Implement modular validation framework
- Deploy HyperlaneValidationModule
- Launch token swap system (xxx-cb ↔ hypxxx)
- Conduct security testing for new validation path
- Gradual migration of routes to Hyperlane ISM

### Phase 7: GameFi & Ecosystem Expansion

**Objective**: Launch entertainment and community features

- Deploy ImmuneDB infrastructure
- Launch PROTOCASS alpha
- Implement creator economy
- Expand supported chains
- Integrate cross-ecosystem partnerships

### Phase 8: Maturity

**Objective**: Achieve full decentralization and self-sustainability

- Complete DAO transition
- Achieve protocol sustainability through fee revenue
- Expand to additional L2s and alternative L1s
- Establish ecosystem grants program
- Community-driven roadmap governance

---

## Conclusion

CL8Y represents a comprehensive vision for blockchain infrastructure that prioritizes security, transparency, and progressive decentralization. By building on proven technologies while introducing innovative mechanisms like UST1 burn economics and oracle-free lending, the ecosystem aims to provide real utility for the Terra Classic community and beyond.

The phased roadmap ensures that each component is thoroughly tested and audited before deployment, while the modular architecture allows for continuous improvement and adaptation to emerging technologies like Hyperlane.

Through careful governance design and community involvement, CL8Y aims to become a self-sustaining ecosystem that demonstrates how blockchain projects can evolve from centralized beginnings to fully decentralized operation without sacrificing security or user experience.

---

## Appendix A: Token Summary

| Token | Type | Purpose |
|-------|------|---------|
| CL8Y | Governance | DAO voting, node staking |
| USTR | Utility | Ecosystem access, referrals |
| UST1 | Stablecoin | Collateralized trading medium |
| xxx-cb | Wrapped | Bridge-wrapped assets |

## Appendix B: Contract Addresses

*To be populated upon mainnet deployment*

## Appendix C: Glossary

| Term | Definition |
|------|------------|
| **ADL** | Auto-Deleveraging - automatic position reduction mechanism |
| **CR** | Collateralization Ratio |
| **ISM** | Interchain Security Module (Hyperlane) |
| **MEV** | Maximal Extractable Value |
| **Watchtower** | Security monitor with cancel capability |

---

*Document Version: 3.0*
*CL8Y Ecosystem*
