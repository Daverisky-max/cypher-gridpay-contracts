#!/usr/bin/env python3
"""
Minimal Cypher GridPay Python quickstart.

Demonstrates the three things a backend almost always needs to do:

  1. Point the SDK at a network.
  2. Build a typed contract client.
  3. Call a read function and a state-changing function.

The generated bindings (see scripts/generate-python-bindings.sh) are plain Python
classes with type hints, so the rest of your service gets static checking too.

Run it:
    ./scripts/generate-python-bindings.sh
    pip install -e bindings/python/payments
    python examples/python/quickstart.py

Environment:
    GRIDPAY_NETWORK   public | testnet        (default: testnet)
    PAYMENT_CONTRACT_ID   C...   contract id of the deployed payment contract
    GRIDPAY_SECRET_KEY    S...   source account (testnet only - see warning below)
"""

import os
import sys

try:
    import stellar_sdk
except ImportError:  # pragma: no cover
    sys.exit("stellar-sdk is not installed. Try: pip install stellar-sdk")

try:
    from payments import PaymentsClient
except ImportError:  # pragma: no cover
    sys.exit(
        "bindings for the payment contract are missing.\n"
        "Generate them first:  ./scripts/generate-python-bindings.sh"
    )

NETWORK = os.environ.get("GRIDPAY_NETWORK", "testnet")
PAYMENT_CONTRACT_ID = os.environ.get("PAYMENT_CONTRACT_ID", "CDUMMYCONTRACTID")
SECRET_KEY = os.environ.get("GRIDPAY_SECRET_KEY", "")

PASSphrase = stellar_sdk.Network.TESTNET_PASSPHRASE if NETWORK == "testnet" \
    else stellar_sdk.Network.PUBLIC_PASSPHRASE


def main() -> int:
    # ------------------------------------------------------------------
    # 1. Build a client bound to one network and one contract.
    # ------------------------------------------------------------------
    if SECRET_KEY:
        keypair = stellar_sdk.Keypair.from_secret_key(SECRET_KEY)
        source = keypair.public_key
    else:
        # Read-only mode: no signing key, only query calls are available.
        source = None
        keypair = None

    server = stellar_sdk.Server(stellar_sdk.Server.TESTNET if NETWORK == "testnet"
                                else stellar_sdk.Server.PUBLIC)
    client = PaymentsClient(source=source, server=server, network=PASSphrase,
                            contract_id=PAYMENT_CONTRACT_ID)

    print(f"network       : {NETWORK}")
    print(f"contract id   : {PAYMENT_CONTRACT_ID}")
    print(f"source account: {source or '(read-only)'}")

    # ------------------------------------------------------------------
    # 2. A typed read call - get_payment() returns a Payment dataclass.
    # ------------------------------------------------------------------
    payment_id = os.environ.get("PAYMENT_ID", "")
    if not payment_id:
        print("\nSet PAYMENT_ID to query a specific payment, e.g.:")
        print("  PAYMENT_ID=<payment id> python examples/python/quickstart.py")
        return 0

    print(f"\n--- get_payment({payment_id}) ---")
    try:
        payment = client.get_payment(payment_id)
        print(f"amount : {getattr(payment, 'amount', '?')}")
        print(f"status : {getattr(payment, 'status', '?')}")
    except stellar_sdk.exceptions.ContractError as exc:
        # ContractError.args[0] is the generated ErrorCode enum value.
        print(f"contract rejected the call: {exc.args[0] if exc.args else exc}")
    except Exception as exc:  # noqa: BLE001 - surfaced verbatim in a quickstart
        print(f"query failed: {exc}")

    # ------------------------------------------------------------------
    # 3. A state-changing call. Requires a signing key.
    # ------------------------------------------------------------------
    if keypair is None:
        print("\nSet GRIDPAY_SECRET_KEY to exercise a state-changing call "
              "(e.g. complete_payment / refund_payment).")
        return 0

    print("\n--- complete_payment ---")
    print("WARNING: this signs a real transaction on the selected network.")
    print("Never point this at public mainnet with a funded key.")
    confirm = os.environ.get("GRIDPAY_CONFIRM", "").lower() in ("1", "yes", "true")
    if not confirm:
        print("Set GRIDPAY_CONFIRM=1 to actually broadcast. Skipping.")
        return 0

    try:
        # Illustrative - the exact call shape is generated from the contract's
        # current interface, so always check bindings/payments/payments.py.
        res = client.complete_payment(payment_id)
        print(f"submitted tx: {res.hash}")
    except Exception as exc:  # noqa: BLE001
        print(f"submit failed: {exc}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
