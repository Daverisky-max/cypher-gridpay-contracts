# Python SDK

Cypher GridPay contracts can be consumed from Python with **auto-generated,
type-safe bindings**. Each contract gets its own importable package containing
a `*Client` class, the contract's types as dataclasses, and its errors as enums.

This is what backend services (Django, FastAPI, Celery workers) should use
instead of hand-rolling `invoke_contract` / `simulate_transaction` calls.

## Quick start

```bash
# 1. Generate bindings (builds the contracts to WASM first)
./scripts/generate-python-bindings.sh

# 2. Install the package you need
pip install -e bindings/python/payments

# 3. Run the quickstart
python examples/python/quickstart.py
```

## Generated layout

```
bindings/python/
├── payments/      # from core/contracts/payment       (crate: payments)
├── escrow/        # from core/contracts/escrow        (crate: escrow)
├── refund/        # from core/contracts/refund        (crate: refund)
└── admin/         # from orchestrator/contracts/admin (crate: admin)
```

Each package contains roughly:

| File | Contents |
|---|---|
| `<contract>.py` | `<Contract>Client` with one method per contract function, correctly typed. |
| `types.py` | Structs and enums as dataclasses. |
| `errors.py` | The contract's `#[contracterror]` variants as an `ErrorCode` enum. |

`bindings/` is generated output and is **not** committed — regenerate it rather
than editing it by hand.

## Using a client

```python
import stellar_sdk
from payments import PaymentsClient
from payments.types import Payment

server = stellar_sdk.Server(stellar_sdk.Server.TESTNET)
keypair = stellar_sdk.Keypair.from_secret_key(secret)   # keep this server-side

client = PaymentsClient(
    source=keypair,
    server=server,
    network=stellar_sdk.Network.TESTNET_PASSPHRASE,
    contract_id=payment_contract_id,
)

payment: Payment | None = client.get_payment(payment_id)
```

`source=None` gives a read-only client for query functions.

## Error handling

Contract errors are raised as `stellar_sdk.exceptions.ContractError`, whose first
argument is the generated `ErrorCode` enum member. Switch on it rather than
string-matching:

```python
from payments.errors import ErrorCode

try:
    client.complete_payment(payment_id)
except stellar_sdk.exceptions.ContractError as exc:
    if exc.args and exc.args[0] == ErrorCode.PaymentNotFound:
        ...
```

`ErrorCode` is generated from the contract's error enum, so it stays in sync with
the deployed contract. The human-readable catalogue lives in
[`ERROR_CODES.md`](ERROR_CODES.md).

## Regenerating after a contract change

Bindings are derived from the compiled WASM, so **regenerate after any contract
interface change** and re-run your type checker:

```bash
./scripts/generate-python-bindings.sh
mypy bindings/python
```

Because the generated code is annotated, a breaking contract change shows up as a
type error in CI rather than a runtime failure in production. This is the main
reason to prefer generated bindings over raw `invoke_transaction` calls.

## Related

- [`DEPLOYMENT.md`](DEPLOYMENT.md) — deploying the contracts these bindings target.
- [`STORAGE_VERSIONING.md`](STORAGE_VERSIONING.md) — migration notes for persisted state.
