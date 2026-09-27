# Python SDK

The Cypher GridPay Python SDK provides typed contract wrappers for querying and managing payments and escrows on the Stellar network.

## Installation

```bash
# Generate the SDK
./scripts/generate-python-bindings.sh

# Install the package
cd python-sdk
pip install -e .
```

## Quickstart

### Initialize the SDK

```python
from cypher_gridpay_sdk import PaymentContract, EscrowContract, RefundContract, AdminContract

rpc_url = "https://soroban-testnet.stellar.org"
network_passphrase = "Test SDF Network ; September 2015"

payment = PaymentContract(
    contract_id="CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4",
    rpc_url=rpc_url,
    network_passphrase=network_passphrase,
)

escrow = EscrowContract(
    contract_id="CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4",
    rpc_url=rpc_url,
    network_passphrase=network_passphrase,
)
```

### Create a Payment

```python
payment_id = payment.create_payment(
    customer="GABC...",
    merchant="GDEF...",
    amount=10000,
    token="CXYZ...",
    currency="USDC",
    expiration_duration=86400,
    metadata="Order #12345",
)
print(f"Payment created with ID: {payment_id}")
```

### Complete a Payment

```python
payment.complete_payment(
    admin="GADMIN...",
    payment_id=payment_id,
)
```

### Query Payment Details

```python
info = payment.get_payment(payment_id=payment_id)
print(f"Status: {info.status}")
print(f"Amount: {info.amount}")
print(f"Merchant: {info.merchant}")
```

### Create an Escrow

```python
escrow_id = escrow.create_escrow(
    customer="GABC...",
    merchant="GDEF...",
    amount=50000,
    token="CXYZ...",
    expiration_duration=604800,
)
```

### Request a Refund

```python
refund_id = refund.request_refund(
    customer="GABC...",
    payment_id=payment_id,
    reason="Product not received",
)
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `STELLAR_RPC_URL` | Soroban RPC endpoint | `https://soroban-testnet.stellar.org` |
| `STELLAR_NETWORK_PASSPHRASE` | Network passphrase | `Test SDF Network ; September 2015` |
| `STELLAR_SECRET_KEY` | Secret key for signing | None |

## Error Handling

```python
from cypher_gridpay_sdk import PaymentContract

try:
    payment_id = payment.create_payment(...)
except Exception as e:
    error_code = getattr(e, 'code', None)
    if error_code == 106:
        print("Rate limit exceeded. Please wait and try again.")
    elif error_code == 108:
        print("Address flagged. Contact support.")
    else:
        print(f"Error: {e}")
```

## API Reference

See the generated Python modules for full API documentation:

- `python-sdk/payment.py` — Payment contract bindings
- `python-sdk/escrow.py` — Escrow contract bindings
- `python-sdk/refund.py` — Refund contract bindings
- `python-sdk/admin.py` — Admin contract bindings
