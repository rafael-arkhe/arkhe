import pytest
from core.crypto import canonical_json_bytes, canonical_json_str

def test_canonical_json_key_sorting():
    obj1 = {"z": 1, "a": 2, "m": {"b": 3, "a": 4}}
    obj2 = {"a": 2, "m": {"a": 4, "b": 3}, "z": 1}
    assert canonical_json_bytes(obj1) == canonical_json_bytes(obj2)
    assert canonical_json_str(obj1) == '{"a":2,"m":{"a":4,"b":3},"z":1}'

def test_canonical_json_no_extra_whitespace():
    obj = {"test": [1, 2, 3], "flag": True}
    res = canonical_json_str(obj)
    assert ": " not in res
    assert ", " not in res
