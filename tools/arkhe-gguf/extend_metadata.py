#!/usr/bin/env python3
"""
extend_metadata.py — injeta os metadados `arkhe.attestation.*` (schema **v1.4**)
num GGUF, produzindo o artefacto final a partir de um GGUF base.

# Porque o schema mudou de v1.3 para v1.4

O v1.3 escrevia **dentro** do `.gguf` os campos `anchored.log_id`,
`anchored.log_index` e `anchored.inclusion_proof`. Esses campos só existem
**depois** de ancorar no Rekor — e ancorar é assinar, e a assinatura tem de
cobrir o ficheiro final. Escrever a prova da sua própria ancoragem dentro do
ficheiro que ela descreve é impossível: ou o ficheiro muda depois de assinado,
ou a assinatura cobre bytes que já não existem.

A v1.4 resolve isso como o OpenSSF Model Signing: o `.gguf` carrega apenas os
**hashes do sujeito**, e a âncora vive no **bundle companheiro** (`.sig`/DSSE),
que é onde o OMS a coloca por design. Por isso este script:

- **não** escreve `anchored.log_id`, `anchored.log_index` nem
  `anchored.inclusion_proof` — não existem no momento da extensão;
- **não** escreve `signed_at` — o instante da assinatura pertence ao bundle, e
  aqui é desconhecido. O instante desta extensão é `verifiable.extended_at`;
- **declara** `anchored.external = true` e aponta `anchored.bundle_uri` para o
  bundle que transportará a âncora.

# O que o script faz, e o que ele não faz

Lê o GGUF base, **re-serializa todos os pares chave-valor que ele já tem sem os
alterar**, acrescenta as chaves `arkhe.attestation.*` da tabela abaixo, e
reescreve os descritores de tensor e o bloco de dados de pesos sem os tocar. O
contador de pares chave-valor sobe exactamente pelo número de chaves novas.

O ficheiro de **entrada nunca é aberto para escrita** (o `GGUFWriter` escreve
noutro caminho), e o script recusa-se a correr se entrada e saída forem o mesmo
caminho.

# Chaves escritas

| Chave (`arkhe.attestation.` +) | Tipo | Valor |
|:---|:---|:---|
| `schema_version` | STRING | `1.4` |
| `source.file_hashes.sha256` | STRING | SHA-256 do ficheiro base |
| `source.file_hashes.blake3` | STRING | BLAKE3 do ficheiro base (omitida se o `blake3` não passar no auto-teste) |
| `source.size_bytes` | UINT64 | tamanho do base em bytes |
| `verifiable.extended_at` | STRING | instante desta extensão, ISO 8601 UTC (`...Z`) |
| `attested.attestation_type` | STRING | `PROMISE` |
| `attested.declared_capabilities` | ARRAY[STRING] | vazio |
| `attested.not_capabilities` | ARRAY[STRING] | `code-execution`, `network-access`, `file-write`, `shell-execution` |
| `attested.disclaimer` | STRING | as capacidades são declarações do signatário, não verificações |
| `revocation.revocable_at` | STRING | `https://arkhe.computer/revocation.json` |
| `revocation.strategy` | ARRAY[STRING] | `short_lived` |
| `anchored.external` | BOOL | `true` |
| `anchored.anchor_type` | STRING | `rekor` |
| `anchored.bundle_uri` | STRING | `arkhe.gguf.sig` |

Nenhum valor é inventado: o SHA-256, o BLAKE3 e o tamanho são calculados do
ficheiro base; o instante é lido do relógio. Se o `blake3` não estiver
disponível ou não passar no auto-teste, a chave é **omitida** e o script di-lo —
nunca escreve um digest que não mediu.

# Duas notas sobre a biblioteca `gguf` (0.19.0), ambas medidas

1. **`GGUFWriter` recusa arrays vazios.** `_pack_val` levanta
   `ValueError("Invalid GGUF metadata array. Empty array")` quando `len(val) == 0`,
   e o atalho `add_array()` ainda descarta o campo antes disso. O formato GGUF
   *permite* um array vazio (tipo do elemento em `u32`, contagem `u64 = 0`), e
   `attested.declared_capabilities` é declarado vazio de propósito — é a postura
   honesta. Por isso `ArkheGgufWriter` sobrepõe apenas esse caso, mantendo o
   resto da serialização da biblioteca.
2. **`GGUFReader` não tem `.arch`** nesta versão (o script v1.3 usava
   `reader.arch`). A arquitectura lê-se do campo `general.architecture`.

# Ambiente

O `PYTHONPATH` não chega a este intérprete, então o directório das bibliotecas é
inserido em `sys.path` explicitamente: primeiro por `ARKHE_GGUF_PYLIBS`, depois
pela localização convencional do workspace do agente. Ver `_ensure_gguf_importable`.

Uso:

    python3 extend_metadata.py <base.gguf> <arkhe.gguf>
"""

from __future__ import annotations

import hashlib
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable

SCHEMA_VERSION = "1.4"

ANCHOR_BUNDLE_URI = "arkhe.gguf.sig"

#: Os três pseudo-campos que o `GGUFReader` expõe em `fields` mas que **não** são
#: pares chave-valor do ficheiro: são o cabeçalho já interpretado. Contá-los como
#: pares faria o contador de kv subir a mais, e re-escrevê-los criaria chaves
#: literais `GGUF.version` no ficheiro.
HEADER_PSEUDO_FIELDS = frozenset({"GGUF.version", "GGUF.tensor_count", "GGUF.kv_count"})

#: A chave da arquitectura. O construtor do `GGUFWriter` acrescenta-a sozinho
#: (`add_architecture`), logo o ciclo de cópia não a volta a escrever — só
#: confirma que o valor que o construtor pôs é o do base.
ARCHITECTURE_KEY = "general.architecture"

#: Directório convencional das bibliotecas do workspace do agente. Não é um
#: `pip install` global: é onde este ambiente as tem. Só é consultado se
#: `import gguf` falhar e `ARKHE_GGUF_PYLIBS` não estiver definido.
_PYLIBS_FALLBACK = (
    Path.home()
    / ".openclaw-autoclaw"
    / "agents"
    / "auto-coder"
    / "workspace"
    / ".openclaw"
    / "tmp"
    / "pylibs"
)


def _ensure_gguf_importable() -> None:
    """Garante que `import gguf` funciona, inserindo o directório das libs em
    `sys.path` se for preciso.

    O `PYTHONPATH` não é lido por este intérprete, por isso a inserção é
    explícita. Ordem: `import gguf` normal, depois `ARKHE_GGUF_PYLIBS`, depois
    `_PYLIBS_FALLBACK`. Se nada resultar, o `ImportError` sobe com a mensagem do
    próprio Python.
    """
    try:
        import gguf  # noqa: F401
        return
    except ImportError:
        pass

    candidates: list[Path] = []
    env = os.environ.get("ARKHE_GGUF_PYLIBS")
    if env:
        candidates.append(Path(env))
    candidates.append(_PYLIBS_FALLBACK)

    for candidate in candidates:
        if (candidate / "gguf" / "__init__.py").is_file():
            sys.path.insert(0, str(candidate))
            import gguf  # noqa: F401
            return

    raise ImportError(
        "o módulo `gguf` não foi encontrado. Instale-o (`pip install gguf`) ou "
        "aponte ARKHE_GGUF_PYLIBS para o directório que contém a pasta `gguf/`. "
        f"Procurado em: {', '.join(str(c) for c in candidates)}"
    )


_ensure_gguf_importable()

from gguf import GGUFReader, GGUFValueType, GGUFWriter  # noqa: E402

#: Os dois vectores de teste do BLAKE3 (vazio e `abc`), publicados com a
#: especificação. Servem para provar que o digest que escrevemos não é um valor
#: de uma implementação que não confere com a referência.
_BLAKE3_TEST_VECTORS = {
    b"": "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
    b"abc": "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85",
}


class ArkheGgufWriter(GGUFWriter):
    """`GGUFWriter` com uma única correcção: arrays vazios.

    O `_pack_val` da biblioteca recusa `len(val) == 0`. O formato GGUF não: um
    array vazio é o tipo do elemento (`u32`) seguido da contagem (`u64 = 0`), sem
    itens. `attested.declared_capabilities: []` é um array vazio **declarado de
    propósito**, por isso o caso tem de ser serializável.

    Tudo o resto da serialização continua a ser o da biblioteca — este override
    não reimplementa o escritor.
    """

    def _pack_val(  # type: ignore[override]
        self,
        val: Any,
        vtype: GGUFValueType,
        add_vtype: bool,
        sub_type: GGUFValueType | None = None,
    ) -> bytes:
        if vtype == GGUFValueType.ARRAY and isinstance(val, (list, tuple)) and len(val) == 0:
            if sub_type is None:
                raise ValueError(
                    "array vazio sem tipo de elemento declarado: um array vazio em GGUF "
                    "não tem itens de onde inferir o tipo, por isso o `sub_type` é obrigatório"
                )
            packed = bytearray()
            if add_vtype:
                packed += self._pack("I", vtype)
            packed += self._pack("I", sub_type)
            packed += self._pack("Q", 0)
            return bytes(packed)
        return super()._pack_val(val, vtype, add_vtype, sub_type)


def sha256_file(path: Path) -> str:
    """SHA-256 do ficheiro inteiro, em streaming (o ficheiro tem ~1,6 GB)."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 22), b""):
            digest.update(chunk)
    return digest.hexdigest()


def blake3_file(path: Path) -> str | None:
    """BLAKE3 do ficheiro inteiro, ou `None` se não for calculável de forma
    verificável.

    Antes de medir o ficheiro, a implementação é confrontada com os dois vectores
    de teste da especificação. Uma implementação que não os reproduz não é usada
    para escrever nada — um digest errado num artefacto é pior do que a ausência
    do campo.
    """
    try:
        import blake3
    except ImportError:
        print("BLAKE3:     omitido — o módulo `blake3` não está instalado (pip install blake3)")
        return None

    for message, expected in _BLAKE3_TEST_VECTORS.items():
        actual = blake3.blake3(message).hexdigest()
        if actual != expected:
            print(
                f"BLAKE3:     omitido — a implementação não reproduz o vector de teste "
                f"({message!r}: esperado {expected}, obtido {actual})"
            )
            return None

    digest = blake3.blake3()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 22), b""):
            digest.update(chunk)
    return digest.hexdigest()


def attestation_fields(
    base_sha256: str,
    base_blake3: str | None,
    base_size: int,
    extended_at: str,
) -> list[tuple[str, GGUFValueType, GGUFValueType | None, Any]]:
    """Os pares `arkhe.attestation.*` a acrescentar, na ordem em que são escritos.

    Devolve tuplos `(chave, tipo, sub-tipo, valor)`. O sub-tipo só é usado em
    arrays e é obrigatório para o array vazio.
    """
    prefix = "arkhe.attestation."
    string = GGUFValueType.STRING
    array = GGUFValueType.ARRAY

    fields: list[tuple[str, GGUFValueType, GGUFValueType | None, Any]] = [
        (prefix + "schema_version", string, None, SCHEMA_VERSION),
        (prefix + "source.file_hashes.sha256", string, None, base_sha256),
    ]

    # Só se o BLAKE3 tiver sido medido, e medido por uma implementação que passa
    # nos vectores de teste. Omitir é a resposta honesta; um placeholder não é.
    if base_blake3 is not None:
        fields.append((prefix + "source.file_hashes.blake3", string, None, base_blake3))

    fields += [
        (prefix + "source.size_bytes", GGUFValueType.UINT64, None, base_size),
        (prefix + "verifiable.extended_at", string, None, extended_at),
        (prefix + "attested.attestation_type", string, None, "PROMISE"),
        # Vazio, e declarado vazio: é a postura honesta.
        (prefix + "attested.declared_capabilities", array, string, []),
        (
            prefix + "attested.not_capabilities",
            array,
            string,
            ["code-execution", "network-access", "file-write", "shell-execution"],
        ),
        (
            prefix + "attested.disclaimer",
            string,
            None,
            "Capabilities são declarações do signatário, não verificações.",
        ),
        (prefix + "revocation.revocable_at", string, None, "https://arkhe.computer/revocation.json"),
        (prefix + "revocation.strategy", array, string, ["short_lived"]),
        # A âncora vive no bundle companheiro, não aqui: é isso que a v1.4 diz.
        (prefix + "anchored.external", GGUFValueType.BOOL, None, True),
        (prefix + "anchored.anchor_type", string, None, "rekor"),
        (prefix + "anchored.bundle_uri", string, None, ANCHOR_BUNDLE_URI),
    ]

    return fields


def copy_existing_key_values(reader: GGUFReader, writer: GGUFWriter) -> int:
    """Re-escreve todos os pares chave-valor do base **sem os alterar**.

    Devolve quantos pares foram copiados. Os três pseudo-campos do cabeçalho
    (`HEADER_PSEUDO_FIELDS`) são saltados: não são pares do ficheiro, e escrevê-los
    criaria chaves literais `GGUF.version`, `GGUF.tensor_count` e `GGUF.kv_count`.

    O tipo é preservado a partir de `ReaderField.types` — `contents()` devolve o
    valor já convertido, não o tipo. Em arrays, `types[-1]` é o tipo do elemento.

    `general.architecture` também é saltada no ciclo, porque o construtor do
    `GGUFWriter` já a escreveu — reescrevê-la só produziria um aviso de chave
    duplicada. O valor é conferido contra o do base em vez de assumido, e o par
    conta como copiado: ele está no ficheiro, escrito pelo construtor.
    """
    copied = 0
    for key, field in reader.fields.items():
        if key in HEADER_PSEUDO_FIELDS:
            continue
        value = field.contents()
        if key == ARCHITECTURE_KEY:
            if value != writer.arch:
                raise SystemExit(
                    f"arquitectura do base (`{value}`) não confere com a que o escritor "
                    f"declarou (`{writer.arch}`)"
                )
            copied += 1
            continue
        vtype = field.types[0]
        sub_type = field.types[-1] if vtype == GGUFValueType.ARRAY else None
        writer.add_key_value(key, value, vtype, sub_type=sub_type)
        copied += 1
    return copied


def copy_tensors(reader: GGUFReader, writer: GGUFWriter) -> int:
    """Re-escreve todos os tensores do base, com os mesmos nomes, formas e tipos.

    Duas notas, ambas verificadas contra a fonte da `gguf` 0.19.0:

    - `ReaderTensor.shape` está na **ordem do disco** (`gguf_reader.py:363` guarda
      `dims` como lidos), enquanto `GGUFWriter.write_ti_data_to_file` escreve
      `ti.shape[n_dims - 1 - j]`, isto é, ao contrário (`gguf_writer.py:268`).
      Passar `shape` directamente inverteria as dimensões.
    - `ReaderTensor.data` é uma vista `memmap`, e para tipos quantizados tem a
      **forma em bytes** (`gguf_reader.py:359`). Passá-la como `raw_shape` com o
      `raw_dtype` faz `add_tensor_info` convertê-la de volta para a forma lógica
      (`gguf_writer.py:360`), que é exactamente o que o escritor espera.

    Como `use_temp_file=False`, as vistas não são carregadas para memória em
    bloco: cada tensor é escrito em streaming por `tofile`.
    """
    for tensor in reader.tensors:
        writer.add_tensor(
            tensor.name,
            tensor.data,
            raw_shape=tensor.data.shape,
            raw_dtype=tensor.tensor_type,
        )
    return len(reader.tensors)


def extend(
    input_path: Path,
    output_path: Path,
    now: Callable[[], datetime] = lambda: datetime.now(timezone.utc),
) -> dict[str, Any]:
    """Produz `output_path` a partir de `input_path`, com o schema v1.4.

    Devolve um dicionário com os factos medidos, para quem chamar os poder
    reportar sem os recalcular.
    """
    if input_path.resolve() == output_path.resolve():
        raise SystemExit(
            "entrada e saída são o mesmo caminho: o base é o sujeito da atestação e "
            "não pode ser reescrito"
        )
    if not input_path.is_file():
        raise SystemExit(f"ficheiro de entrada inexistente: {input_path}")
    if output_path.exists():
        raise SystemExit(
            f"a saída já existe: {output_path} — apague-a à mão se quiser regerar"
        )

    print(f"base:       {input_path}")
    base_sha256 = sha256_file(input_path)
    base_size = input_path.stat().st_size
    base_blake3 = blake3_file(input_path)
    print(f"SHA-256:    {base_sha256}")
    print(f"BLAKE3:     {base_blake3 if base_blake3 else '(omitido)'}")
    print(f"tamanho:    {base_size} bytes")

    extended_at = now().astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    reader = GGUFReader(input_path)

    # `GGUFWriter.__init__` acrescenta `general.architecture` por si. Passar-lhe a
    # arquitectura lida do base faz com que o valor coincida; a chave já existe,
    # logo mantém a posição que tinha (o `dict` preserva a ordem de inserção).
    architecture = str(reader.fields["general.architecture"].contents())
    writer = ArkheGgufWriter(output_path, architecture)

    # O alinhamento declarado pelo base governa o padding antes do bloco de dados.
    # Se o escritor usasse outro, um leitor que calculasse `data_offset` a partir
    # do `general.alignment` do ficheiro leria no sítio errado.
    writer.data_alignment = reader.alignment

    copied_keys = copy_existing_key_values(reader, writer)
    print(f"kv do base: {copied_keys} pares copiados (o base declara {len(reader.fields) - len(HEADER_PSEUDO_FIELDS)})")

    new_fields = attestation_fields(base_sha256, base_blake3, base_size, extended_at)
    for key, vtype, sub_type, value in new_fields:
        writer.add_key_value(key, value, vtype, sub_type=sub_type)
    print(f"kv novos:   {len(new_fields)} pares arkhe.attestation.* (schema {SCHEMA_VERSION})")

    copied_tensors = copy_tensors(reader, writer)
    print(f"tensores:   {copied_tensors} copiados")

    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_tensors_to_file()
    writer.close()

    output_sha256 = sha256_file(output_path)
    output_size = output_path.stat().st_size

    # O base foi aberto só para leitura, mas isso é uma intenção; isto é a medida.
    base_rehash = sha256_file(input_path)
    base_unchanged = base_rehash == base_sha256

    print()
    print(f"arkhe.gguf: {output_path}")
    print(f"tamanho:    {output_size} bytes")
    print(f"SHA-256:    {output_sha256}")
    print(f"base intacto: {'sim' if base_unchanged else 'NÃO'} ({base_rehash})")

    if not base_unchanged:
        raise SystemExit(
            "o ficheiro base mudou durante a execução: o artefacto não pode ser "
            "apresentado como atestação sobre ele"
        )

    return {
        "input": str(input_path),
        "output": str(output_path),
        "base_sha256": base_sha256,
        "base_blake3": base_blake3,
        "base_size": base_size,
        "output_sha256": output_sha256,
        "output_size": output_size,
        "copied_keys": copied_keys,
        "new_keys": [key for key, _, _, _ in new_fields],
        "new_keys_written": len(new_fields),
        "copied_tensors": copied_tensors,
        "extended_at": extended_at,
    }


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("uso: extend_metadata.py <base.gguf> <arkhe.gguf>", file=sys.stderr)
        sys.exit(2)

    extend(Path(sys.argv[1]), Path(sys.argv[2]))
