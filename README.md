# AMM

A constant-product AMM using Anchor and legacy SPL Token for Turbin3.

## Instructions

- Initialize: create the pool, LP mint, reserve vaults and treasury accounts.
- Deposit: provide liquidity and mint LP shares. The first depositor sets the initial amounts and share supply. Later deposits use the reserve ratio, rounded up.
- Withdraw: burn LP shares and receive proportional reserves, rounded down.
- Swap: exchange either token, subject to a minimum output.

## Fees

The swap fee is configurable in basis points (0–9,999). The treasury receives 20% of the rounded fee in the input token; the remaining fee stays in the pool for liquidity providers. The initializer controls the two treasury token accounts, which are separate from the reserves.

For input `a`, input reserve `x`, output reserve `y`, and fee `f` in basis points:

```text
fee = floor(a * f / 10000)
net = a - fee
output = floor(y * net / (x + net))
treasury = floor(fee / 5)
```

The curve uses integer arithmetic with u128 intermediates, without an external curve library.

## Build and test

Use Anchor CLI 1.0.1, Agave 3.1.8, and the Rust version pinned in `rust-toolchain.toml`.

Run the build and tests with a program keypair matching the declared ID.

```bash
anchor build && cargo test --workspace
```

On a fresh checkout, use `anchor build --ignore-keys && cargo test --workspace` to run LiteSVM against the declared ID without the deployment keypair. For a disposable local deployment, use `anchor keys sync` first to match the generated local keypair.

The LiteSVM tests cover all four instructions, slippage, invalid fees, substituted accounts, swaps in both directions, and user, reserve, and treasury balances. Unit tests cover math and rounding. The integration tests require the SBF artifact from `anchor build`.

## Downtime mitigation

Use multiple RPC providers and check slot freshness before quoting. Keep pool state on-chain and enforce slippage in the program. Retry the same signed transaction before blockhash expiry, and check its status before re-signing. Pause new quotes when chain data is stale and reconcile transactions after recovery. Multiple RPC providers cannot overcome a chain-wide outage.

This is an educational implementation, not an audited production AMM. Treasury accounts must remain open for swaps.
