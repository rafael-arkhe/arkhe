import json
import pytest
from core.crypto import AfirReceipt, MLDSA65Simulator, canonical_json_bytes

def test_valid_afir_schema():
    with open("vectors/afir/valid_receipt.json") as f:
        data = json.load(f)
    receipt = AfirReceipt(data)
    assert receipt.data["epoch"] == 100
    assert receipt.verify_signature("mldsa65-pub:worker-alpha")

def test_missing_fields_raises_error():
    bad_data = {
        "receipt_id": "r1",
        "epoch": 1,
    }
    with pytest.raises(ValueError, match="missing required fields"):
        AfirReceipt(bad_data)

def test_invalid_epoch_raises_error():
    with open("vectors/afir/valid_receipt.json") as f:
        data = json.load(f)
    data["epoch"] = -5
    with pytest.raises(ValueError, match="epoch must be non-negative"):
        AfirReceipt(data)
