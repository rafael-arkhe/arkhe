import random

def fuzz_json_payload(base: dict) -> dict:
    fuzzed = dict(base)
    mutation_type = random.choice(["remove_key", "null_key", "type_swap", "overflow"])
    if mutation_type == "remove_key" and fuzzed:
        k = random.choice(list(fuzzed.keys()))
        del fuzzed[k]
    elif mutation_type == "null_key" and fuzzed:
        k = random.choice(list(fuzzed.keys()))
        fuzzed[k] = None
    elif mutation_type == "type_swap" and fuzzed:
        k = random.choice(list(fuzzed.keys()))
        fuzzed[k] = 123456 if isinstance(fuzzed[k], str) else "malformed_string"
    elif mutation_type == "overflow" and fuzzed:
        k = random.choice(list(fuzzed.keys()))
        fuzzed[k] = 2**64
    return fuzzed
