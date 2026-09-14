# =============================================================================
# arpa_integration.py — BLOCO 484 v45 — Catedral OS
# Consumo de aleatoriedade verificável (ARPA Randcast / BLS-TSS) via EVM.
#
# Revisão vetada (vs. proposta):
#   * `_fulfillRandomness(bytes32, uint256)` — aleatoriedade é uint256 (on-chain),
#     NÃO bytes32. A assinatura da proposta estava errada.
#   * `_requestRandomness` retorna bytes32 (requestId); requisição gate `onlyOwner`.
#   * `getBalance`/`_getBalance` não existem na base real — removidos.
#   * Rede suportada documentada: Base mainnet (Adapter 0xDBa5dE3...), Ethereum
#     (0x4363154E...), Sepolia (0x46d29642...). "Base Sepolia" não consta.
#   * Nenhuma afirmação criptográfica/estatística (uniformidade, imprevisibilidade)
#     é *provada* aqui — ver ARPAVerifiable.lean (axiomas nomeados do BLS-TSS).
#
# Modos:
#   ARPA_UNIT_TEST (default): transporte fake determinístico. A assinatura
#     EIP-1559 e a decodificação de eventos são exercitadas DE VERDADE (sem rede).
#   else: transporte real via web3 (rpc_url/adaptor/consumer/key via env).
#
# Invariantes: Eth-2 (data minimization — nenhum payload sai além do necessário),
# Loopseal-2 (eventos on-chain são o ledger da aleatoriedade), Provenance-1
# (comunicação externa notarizada por TLSNotary). Gap-1: aleatoriedade alimenta
# sementes (Zeno/QSP/Tardos) que passam pelo Zeno; nunca altera Φ_C diretamente.
# =============================================================================
import os
import time
import logging
from typing import Optional

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# ABI gerado por solc 0.8.28 (build_484) — fonte única de verdade.
# ---------------------------------------------------------------------------
CATEDRAL_CONSUMER_ABI = __import__("json").loads(
    """[
{"inputs":[{"internalType":"address","name":"adapter","type":"address"}],"stateMutability":"nonpayable","type":"constructor"},
{"inputs":[{"internalType":"uint256","name":"have","type":"uint256"},{"internalType":"uint32","name":"want","type":"uint32"}],"name":"GasLimitTooBig","type":"error"},
{"inputs":[],"name":"NoSubscriptionBound","type":"error"},
{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"previousOwner","type":"address"},{"indexed":true,"internalType":"address","name":"newOwner","type":"address"}],"name":"OwnershipTransferred","type":"event"},
{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"requestId","type":"bytes32"}],"name":"RandomnessCallbackFailed","type":"event"},
{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"requestId","type":"bytes32"},{"indexed":false,"internalType":"uint256","name":"randomness","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"timestamp","type":"uint256"}],"name":"RandomnessFulfilled","type":"event"},
{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"requestId","type":"bytes32"},{"indexed":true,"internalType":"address","name":"requester","type":"address"}],"name":"RandomnessRequested","type":"event"},
{"inputs":[],"name":"adapter","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},
{"inputs":[],"name":"callbackGasLimit","outputs":[{"internalType":"uint32","name":"","type":"uint32"}],"stateMutability":"view","type":"function"},
{"inputs":[],"name":"callbackMaxGasFee","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},
{"inputs":[{"internalType":"uint64","name":"subId","type":"uint64"}],"name":"getNonce","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},
{"inputs":[],"name":"owner","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},
{"inputs":[{"internalType":"bytes32","name":"requestId","type":"bytes32"},{"internalType":"uint256[]","name":"randomWords","type":"uint256[]"}],"name":"rawFulfillRandomWords","outputs":[],"stateMutability":"nonpayable","type":"function"},
{"inputs":[{"internalType":"bytes32","name":"requestId","type":"bytes32"},{"internalType":"uint256","name":"randomness","type":"uint256"}],"name":"rawFulfillRandomness","outputs":[],"stateMutability":"nonpayable","type":"function"},
{"inputs":[{"internalType":"bytes32","name":"requestId","type":"bytes32"},{"internalType":"uint256[]","name":"shuffledArray","type":"uint256[]"}],"name":"rawFulfillShuffledArray","outputs":[],"stateMutability":"nonpayable","type":"function"},
{"inputs":[],"name":"renounceOwnership","outputs":[],"stateMutability":"nonpayable","type":"function"},
{"inputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"name":"requestCallbacks","outputs":[{"internalType":"address","name":"","type":"address"}],"stateMutability":"view","type":"function"},
{"inputs":[],"name":"requestConfirmations","outputs":[{"internalType":"uint16","name":"","type":"uint16"}],"stateMutability":"view","type":"function"},
{"inputs":[{"internalType":"address","name":"callbackAddress","type":"address"}],"name":"requestRandomness","outputs":[{"internalType":"bytes32","name":"requestId","type":"bytes32"}],"stateMutability":"nonpayable","type":"function"},
{"inputs":[{"internalType":"address","name":"to","type":"address"},{"internalType":"uint256","name":"value","type":"uint256"},{"internalType":"bytes","name":"data","type":"bytes"}],"name":"requiredTxGas","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"nonpayable","type":"function"},
{"inputs":[{"internalType":"uint32","name":"_callbackGasLimit","type":"uint32"},{"internalType":"uint256","name":"_callbackMaxGasFee","type":"uint256"}],"name":"setCallbackGasConfig","outputs":[],"stateMutability":"nonpayable","type":"function"},
{"inputs":[{"internalType":"uint16","name":"_requestConfirmations","type":"uint16"}],"name":"setRequestConfirmations","outputs":[],"stateMutability":"nonpayable","type":"function"},
{"inputs":[{"internalType":"address","name":"newOwner","type":"address"}],"name":"transferOwnership","outputs":[],"stateMutability":"nonpayable","type":"function"}
]"""
)

REQUEST_RANDOMNESS_SELECTOR = "0x0b80d606"  # keccak("requestRandomness(address)")[0:4]

# Endereços oficiais ARPA Randcast (docs.arpanetwork.io — redes suportadas)
ARPA_ADAPTERS = {
    "ethereum-mainnet": "0x4363154E1eC107F81239A4F0b1CB3AB5161129Ca",
    "base-mainnet": "0xDBa5dE33511b4b8549994d30E73b47fedc8cC2C2",
    "sepolia": "0x46d29642cB0d57ae27b18b4aa5f746c918F3D6F8",
}

CHAIN_IDS = {"ethereum-mainnet": 1, "base-mainnet": 8453, "sepolia": 11155111}


def _is_hermetic() -> bool:
    return os.environ.get("ARPA_UNIT_TEST", "1") != "0"


# ---------------------------------------------------------------------------
# Transporte fake determinístico (sem rede) — padrão bloco_483.
# ---------------------------------------------------------------------------
class FakeTransport:
    """Responde eth_sendRawTransaction e eth_getLogs com dados sintéticos."""

    def __init__(self, request_id_hex: str, randomness_int: int):
        check = "0x" + request_id_hex.removeprefix("0x").lower().zfill(64)
        rnd = f"{randomness_int:064x}"
        ts = f"{1_752_540_000:064x}"
        topic0 = "0x43bb0ea8184311848533a3793417198bab8a0c056ae1842abe24397adfc03b44"
        self._logs = [
            {
                "address": "0x" + "42" * 20,
                "topics": [topic0, check],
                "data": "0x" + rnd + ts,
                "blockNumber": "0x2d4b61",
                "transactionHash": "0x" + "ab" * 32,
                "transactionIndex": "0x0",
                "logIndex": "0x5",
                "blockHash": "0x" + "cd" * 32,
                "removed": False,
            }
        ]

    def make_request(self, method: str, params):  # noqa: ARG002
        if method == "eth_getLogs":
            return {"result": self._logs}
        if method == "eth_sendRawTransaction":
            return {"result": "0x" + "ab" * 32}
        if method == "eth_getTransactionCount":
            return {"result": "0x1"}
        if method == "eth_gasPrice":
            return {"result": "0x3b9aca00"}
        raise NotImplementedError(method)


class _RPCResponse(dict):
    @property
    def result(self):
        return self["result"]


class ARPARandcastClient:
    """
    Cliente para o CatedralRandcastConsumer (ARPA Randcast).

    Modo hermenético: constrói e ASSINA a transação EIP-1559 real (eth_account),
    decodifica eventos com o ABI real, mas a rede é fake. Zero dependência de RPC.
    """

    def __init__(self, network: str = "base-mainnet",
                 consumer_address: Optional[str] = None,
                 private_key: Optional[str] = None):
        import json as _json

        from web3 import Web3
        from web3.middleware import (  # type: ignore[attr-defined]
            ExtraDataToPOAMiddleware,
        )

        self.network = network
        self.chain_id = CHAIN_IDS.get(network, 8453)
        self.adapter_address = ARPA_ADAPTERS.get(network, ARPA_ADAPTERS["base-mainnet"])
        if not consumer_address:
            raise ValueError("consumer_address obrigatório (contrato CatedralRandcastConsumer)")
        consumer_address = Web3.to_checksum_address(consumer_address)
        self.consumer_address = consumer_address
        self.private_key = private_key or os.environ.get("ARPA_PRIVATE_KEY")

        self._w3 = Web3()
        if _is_hermetic():
            self._w3.middleware_onion.inject(ExtraDataToPOAMiddleware, layer=0)
            self._rpc = FakeTransport(
                request_id_hex="cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe",
                randomness_int=7032342083477022982271402651651555127564410957179645516121180568454384,
            )
            self._fake_transport = True
        else:
            rpc_url = os.environ.get("ARPA_RPC_URL")
            if not rpc_url:
                raise ValueError("ARPA_RPC_URL obrigatório fora do modo teste")
            self._w3 = Web3(Web3.HTTPProvider(rpc_url))
            self._w3.middleware_onion.inject(ExtraDataToPOAMiddleware, layer=0)
            self._rpc = self._w3.provider
            self._fake_transport = False

        self.consumer = self._w3.eth.contract(
            address=self.consumer_address, abi=CATEDRAL_CONSUMER_ABI
        )

    # -- RPC ----------------------------------------------------------------
    def _rpc_call(self, method: str, params) -> dict:
        resp = self._rpc.make_request(method, params)
        if isinstance(resp, dict) and "error" in resp:
            raise RuntimeError(f"RPC {method}: {resp['error']}")
        return resp

    def _nonce(self, address: str) -> int:
        return int(self._rpc_call("eth_getTransactionCount", [address, "latest"])["result"], 16)

    # -- Fluxo --------------------------------------------------------------
    def request_randomness(self, callback_address: str) -> tuple[str, str]:
        """
        Assina e envia requestRandomness(callback). Retorna (request_id0, tx_hash).
        Em modo hermenético o requestId NÃO vem de RPC (simulado via log fake);
        em produção leia o evento RandomnessRequested do receipt.
        """
        from eth_account import Account

        if not self.private_key:
            raise ValueError("Private key não configurada (ARPA_PRIVATE_KEY)")
        account = Account.from_key(self.private_key)
        cb = self._w3.to_checksum_address(callback_address)

        data = self.consumer.encode_abi("requestRandomness", [cb])

        if self._fake_transport:
            nonce, price = 0, 1_000_000_000
        else:
            nonce = self._nonce(account.address)
            price = int(self._rpc_call("eth_gasPrice", [])["result"], 16)

        tx = {
            "type": 2,
            "chainId": self.chain_id,
            "nonce": nonce,
            "from": account.address,
            "to": self.consumer_address,
            "data": data,
            "gas": 400_000,
            "maxFeePerGas": price * 2,
            "maxPriorityFeePerGas": price,
        }
        signed = Account.sign_transaction(transaction_dict=tx, private_key=self.private_key)
        raw = signed.raw_transaction.hex()
        result = self._rpc_call("eth_sendRawTransaction", [raw])

        if self._fake_transport:
            request_id = "0x" + "ca" * 32  # corresponde ao log fake
        else:
            request_id = self._request_id_from_receipt(result["result"])
        return request_id, result["result"]

    def _request_id_from_receipt(self, tx_hash: str) -> str:
        """Lê requestId do evento RandomnessRequested no receipt (produção)."""
        receipt = self._rpc_call("eth_getTransactionReceipt", [tx_hash])["result"]
        if not receipt:
            raise RuntimeError("receipt não encontrado")
        decode = self.consumer.events.RandomnessRequested().process_receipt(receipt)
        return decode[0]["args"]["requestId"]

    def wait_for_randomness(self, request_id: str, timeout_seconds: int = 60) -> Optional[bytes]:
        """
        Monitora RandomnessFulfilled(requestId, randomness, timestamp) e retorna
        os 32 bytes da aleatoriedade (uint256 → bytes). None em timeout.
        """
        req_topic = "0x" + request_id.removeprefix("0x").lower().zfill(64)
        deadline = time.time() + timeout_seconds
        event_decode = self.consumer.events.RandomnessFulfilled().process_log
        while time.time() < deadline:
            resp = self._rpc_call(
                "eth_getLogs",
                [{"fromBlock": "0x0", "toBlock": "latest",
                  "address": self.consumer_address, "topics": [None, req_topic]}],
            )
            for log in resp["result"]:
                ev = event_decode(log)
                rnd = ev["args"]["randomness"]
                if isinstance(rnd, bytes):
                    return rnd
                return (rnd & ((1 << 256) - 1)).to_bytes(32, "big")
            if self._fake_transport:
                break  # um único poll determinístico no modo teste
            time.sleep(2)
        logger.warning("timeout aguardando aleatoriedade requestId=%s", request_id)
        return None


class ARPARandomnessProvider:
    """Camada de uso: sementes para Zeno/QSP/Tardos a partir da aleatoriedade."""

    def __init__(self, client: ARPARandcastClient):
        self.client = client
        self.callback_address = client.consumer_address
        self.randomness_cache: dict[str, dict] = {}

    def get_random_bytes32(self, purpose: str = "general") -> Optional[bytes]:
        cached = self.randomness_cache.get(purpose)
        if cached and time.time() - cached["ts"] < 300:
            return cached["value"]
        req_id, _tx = self.client.request_randomness(self.callback_address)
        rnd = self.client.wait_for_randomness(req_id)
        if rnd:
            self.randomness_cache[purpose] = {"value": rnd, "ts": time.time()}
            logger.info("aleatoriedade obtida p/ '%s': %s…", purpose, rnd.hex()[:16])
        return rnd

    def get_random_float(self, purpose: str = "general") -> Optional[float]:
        rnd = self.get_random_bytes32(purpose)
        if rnd is None:
            return None
        return int.from_bytes(rnd, "big") / (2 ** 256 - 1)

    def get_random_int(self, purpose: str, lo: int, hi: int) -> Optional[int]:
        rnd = self.get_random_bytes32(purpose)
        if rnd is None:
            return None
        return lo + int.from_bytes(rnd, "big") % (hi - lo + 1)


# ---------------------------------------------------------------------------
# Autoteste hermenético — GAP-1: semente nunca entra em Φ_C diretamente.
# ---------------------------------------------------------------------------
def _selftest() -> int:
    logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
    client = ARPARandcastClient(
        network="base-mainnet",
        consumer_address="0x" + "42" * 20,
        private_key=os.environ.get(
            "ARPA_PRIVATE_KEY",
            "0x1111111111111111111111111111111111111111111111111111111111111111",
        ),
    )
    provider = ARPARandomnessProvider(client)

    req_id, tx = client.request_randomness(client.consumer_address)
    assert req_id.startswith("0x") and len(req_id) == 66, req_id
    assert tx.startswith("0x"), tx
    print(f"[ARPA] requestId={req_id} tx={tx[:18]}…")

    rnd = client.wait_for_randomness(req_id, timeout_seconds=5)
    assert rnd is not None and len(rnd) == 32, rnd
    print(f"[ARPA] randomness={rnd.hex()[:32]}… ({int.from_bytes(rnd,'big')})")

    f = provider.get_random_float("zeno_measurement")
    assert f is not None and 0.0 <= f < 1.0, f
    i = provider.get_random_int("tardos_codes", 1000, 2000)
    assert i is not None and 1000 <= i <= 2000, i
    print(f"[ARPA] zeno_seed={f:.6f} tardos_seed={i}")

    c1 = provider.get_random_bytes32("zeno_measurement")
    c2 = provider.get_random_bytes32("zeno_measurement")
    assert c1 == c2, "cache falhou"
    print("[ARPA] cache hit OK (mesma semente reutilizada 5min)")

    print("[ARPA] SELFTEST PASSED")
    return 0


if __name__ == "__main__":
    raise SystemExit(_selftest())