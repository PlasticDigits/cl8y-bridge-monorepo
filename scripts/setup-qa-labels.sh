#!/usr/bin/env bash
# Creates GitHub labels for QA workflow.
# Usage: ./scripts/setup-qa-labels.sh [owner/repo]
#
# Requires: gh CLI authenticated (gh auth login)

set -euo pipefail

REPO="${1:-$(gh repo view --json nameWithOwner -q .nameWithOwner)}"

echo "Setting up QA labels on ${REPO}..."

declare -A LABELS=(
  # QA workflow
  ["qa"]="0E8A16:QA testing task or result"
  ["test-pass"]="0E8A16:Manual test pass record"
  ["needs-triage"]="FBCA04:Needs initial assessment"
  ["confirmed"]="0075CA:Bug confirmed and reproducible"
  ["cannot-reproduce"]="CCCCCC:Unable to reproduce the reported bug"
  ["in-review"]="5319E7:Fix is under review"

  # Area
  ["frontend"]="1D76DB:Frontend / UI issue"
  ["backend"]="B60205:Backend service issue (operator/canceler)"
  ["smart-contract"]="B60205:Smart contract issue"

  # Type
  ["bug"]="D73A4A:Something is broken"
  ["ux"]="D4C5F9:User experience improvement"
  ["responsive"]="D4C5F9:Mobile / responsive layout issue"

  # Wallet-specific
  ["wallet-issue"]="F9D0C4:Wallet connection or signing issue"
  ["wallet:metamask"]="F9D0C4:MetaMask specific"
  ["wallet:station"]="F9D0C4:Station wallet specific"
  ["wallet:keplr"]="F9D0C4:Keplr specific"
  ["wallet:walletconnect"]="F9D0C4:WalletConnect specific"

  # Device
  ["mobile"]="C2E0C6:Mobile device issue"
  ["desktop"]="C2E0C6:Desktop browser issue"
  ["tablet"]="C2E0C6:Tablet device issue"

  # Priority
  ["P0-critical"]="B60205:Blocks users or risks funds"
  ["P1-high"]="FF6600:Core flow broken"
  ["P2-medium"]="FBCA04:Annoying but has workaround"
  ["P3-low"]="0E8A16:Cosmetic or minor polish"

  # Security
  ["security-escalate"]="B60205:Escalated privately — do not discuss details here"
)

for label in "${!LABELS[@]}"; do
  IFS=':' read -r color desc <<< "${LABELS[$label]}"
  echo "  Creating label: ${label}"
  gh label create "$label" \
    --repo "$REPO" \
    --color "$color" \
    --description "$desc" \
    --force 2>/dev/null || true
done

echo ""
echo "Done. ${#LABELS[@]} labels created/updated on ${REPO}."
