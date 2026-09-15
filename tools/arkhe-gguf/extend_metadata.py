#!/usr/bin/env python3
"""
extend_metadata.py — Injeta metadados arkhe.attestation.* num GGUF assinado.
Após esta etapa, o hash do ficheiro muda (metadados são parte do ficheiro).
O hash final é o que vai para o Rekor.
"""

import sys
import hashlib
from pathlib import Path
from datetime import datetime, timezone

from gguf import GGUFReader, GGUFWriter

def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()

def extend(input_path: Path, output_path: Path, att: dict):
    reader = GGUFReader(input_path)
    writer = GGUFWriter(output_path, reader.arch)

    for key, field in reader.fields.items():
        writer.add_key_value(key, field.contents())

    writer.add_string("arkhe.attestation.schema_version", att["schema_version"])
    writer.add_string("arkhe.attestation.verifiable.file_hashes.sha256", att["file_hashes"]["sha256"])
    writer.add_string("arkhe.attestation.verifiable.file_hashes.blake3", att["file_hashes"]["blake3"])
    writer.add_uint64("arkhe.attestation.verifiable.size_bytes", att["size_bytes"])
    writer.add_string("arkhe.attestation.verifiable.signed_at", att["signed_at"])
    writer.add_string("arkhe.attestation.anchored.anchor_type", att["anchor_type"])
    writer.add_string("arkhe.attestation.anchored.log_id", att["log_id"])
    writer.add_uint64("arkhe.attestation.anchored.log_index", att["log_index"])
    writer.add_string("arkhe.attestation.anchored.inclusion_proof", att["inclusion_proof"])
    writer.add_string("arkhe.attestation.attested.attestation_type", att["attestation_type"])
    writer.add_string(
        "arkhe.attestation.attested.disclaimer",
        "Capabilities são declarações do signatário, não verificações."
    )

    writer.write_header()
    writer.write_kv_data()
    writer.write_tensors()
    writer.close()

    final = sha256_file(output_path)
    print(f"arkhe.gguf: {output_path}")
    print(f"Tamanho:    {output_path.stat().st_size} bytes")
    print(f"SHA-256:    {final}")

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("uso: extend_metadata.py <input.gguf> <output.gguf>")
        sys.exit(1)

    att = {
        "schema_version": "1.2",
        "file_hashes": {"sha256": "<hash base>", "blake3": "<hash b3 base>"},
        "size_bytes": 0,
        "signed_at": datetime.now(timezone.utc).isoformat(),
        "anchor_type": "rekor",
        "log_id": "<log_id>",
        "log_index": 0,
        "inclusion_proof": "<base64>",
        "attestation_type": "PROMISE",
    }

    extend(Path(sys.argv[1]), Path(sys.argv[2]), att)
