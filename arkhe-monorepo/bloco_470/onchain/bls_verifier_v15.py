# =============================================================================
# BLOCO 470 v15 — VERIFICAÇÃO BLS12-381 ON-CHAIN (B1)
# bls_verifier_v15.py
#
# Verificador BLS12-381 on-chain via EIP-2537 (precompile 0x11).
# Ativo no Sepolia após o hardfork Pectra.
#
# Os elementos de campo BLS12-381 (381 bits) não cabem em uint256 nem em
# bytes48 (que não existe em Solidity). Portanto, o input de pareamento é
# montado AQUI, em Python, codificando cada coordenada como 48 bytes
# little-endian, e o contrato (BLS12_381Verifier.sol) apenas valida o
# comprimento e repassa à precompile 0x11.
#
# Requer: pip install web3
# Env:    BLS_VERIFIER_ADDRESS, SEPOLIA_RPC_URL
# =============================================================================
from web3 import Web3
import os
import json
from typing import Dict, List

BLS_PAIRING_INPUT_BYTES = 576  # 2 pares x 288 bytes (1 par = G1(96B) + G2(192B))


class BLSVerifierOnChain:
    """Verificador BLS12-381 on-chain via EIP-2537."""

    def __init__(self, rpc_url: str = None):
        self.rpc_url = rpc_url or os.environ.get("SEPOLIA_RPC_URL", "https://rpc.sepolia.org")
        self.w3 = Web3(Web3.HTTPProvider(self.rpc_url))
        self.verifier_address = os.environ.get("BLS_VERIFIER_ADDRESS")

        if not self.verifier_address:
            raise ValueError("BLS_VERIFIER_ADDRESS é obrigatória (use o deploy do DeployBLSVerifier.s.sol)")

        self.abi = [
            {
                "inputs": [
                    {"internalType": "bytes", "name": "pairingInput", "type": "bytes"},
                    {"internalType": "bytes32[1]", "name": "publicInput", "type": "bytes32[1]"},
                ],
                "name": "verify",
                "outputs": [{"internalType": "bool", "name": "", "type": "bool"}],
                "stateMutability": "nonpayable",
                "type": "function",
            },
            {
                "inputs": [],
                "name": "isEip2537Active",
                "outputs": [{"internalType": "bool", "name": "", "type": "bool"}],
                "stateMutability": "view",
                "type": "function",
            },
        ]

        self.contract = self.w3.eth.contract(
            address=self.verifier_address,
            abi=self.abi,
        )

    # ------------------------------------------------------------------------
    # HELPERS DE CODIFICAÇÃO (elementos de campo → 48 bytes little-endian)
    # ------------------------------------------------------------------------
    @staticmethod
    def _le48(value) -> bytes:
        """Codifica um elemento de campo (< 2^381) como 48 bytes little-endian."""
        v = int(value)
        if v < 0 or v.bit_length() > 381:
            raise ValueError("Elemento de campo BLS12-381 deve estar entre 0 e 2^381 - 1")
        return v.to_bytes(48, "little")

    @classmethod
    def g1_pair_bytes(cls, x, y) -> bytes:
        """G1 = 2 elementos = 96 bytes."""
        return cls._le48(x) + cls._le48(y)

    @classmethod
    def g2_pair_bytes(cls, x0, x1, y0, y1) -> bytes:
        """G2 = 4 elementos (Fp2) = 192 bytes."""
        return cls._le48(x0) + cls._le48(x1) + cls._le48(y0) + cls._le48(y1)

    @classmethod
    def pairing_input(cls, a, b, c, g2_neg) -> str:
        """
        Monta o input EIP-2537 (2 pares, 576 bytes) a partir dos pontos:
          e(a, b) * e(c, -g2) == 1
        a    = [x, y]                          (G1 — assinatura S)
        b    = [x0, x1, y0, y1]                (G2 — chave pública Q)
        c    = [x, y]                          (G1 — hash-to-curve do domínio)
        g2_neg = [x0, x1, y0, y1]              (G2 — negativo do gerador; usar blst/py_ecc)
        Retorna o input como hex "0x...".
        """
        raw = (
            cls.g1_pair_bytes(*a)
            + cls.g2_pair_bytes(*b)
            + cls.g1_pair_bytes(*c)
            + cls.g2_pair_bytes(*g2_neg)
        )
        assert len(raw) == BLS_PAIRING_INPUT_BYTES, "Input de pareamento deve ter 576 bytes"
        return "0x" + raw.hex()

    # ------------------------------------------------------------------------
    # VERIFICAÇÃO
    # ------------------------------------------------------------------------
    def verify(self, proof_data: Dict) -> Dict:
        """
        Verifica prova BLS12-381 on-chain.

        proof_data = {
          "proof": {"pairing_input": "0x...576-byte-encoded..."},  # ou os 4 pontos
          "public_signals": {"phi": 85},
        }

        Se "pairing_input" não for informado, monta-o a partir de
        "a", "b", "c" e "g2_neg" (inteiros decimais).
        """
        try:
            proof = proof_data["proof"]

            if "pairing_input" in proof:
                pairing_hex = proof["pairing_input"]
            else:
                pairing_hex = self.pairing_input(
                    proof["a"], proof["b"], proof["c"], proof["g2_neg"]
                )

            public = proof_data.get("public_signals", {})
            phi = public.get("phi", 0)
            public_input = ["0x" + f"{phi:064x}"]  # bytes32[1]

            result = self.contract.functions.verify(pairing_hex, public_input).call()

            return {
                "verified": bool(result),
                "curve": "BLS12-381",
                "eip": "EIP-2537",
                "network": "sepolia",
                "verifier_address": self.verifier_address,
                "phi": phi,
            }
        except Exception as e:
            return {
                "verified": False,
                "error": str(e),
                "curve": "BLS12-381",
                "eip": "EIP-2537",
            }

    def is_eip2537_active(self) -> bool:
        """
        Delega ao contrato BLS12_381Verifier.isEip2537Active() (15 µcp. view).
        Sem contrato implantado, tenta detectar pela et.eth.call direto.
        """
        try:
            return bool(self.contract.functions.isEip2537Active().call())
        except Exception:
            # Fallback: precompile responde com 32 bytes (produto vazio = 1)?
            try:
                out = self.w3.eth.call({
                    "to": self.w3.to_checksum_address("0x" + "00" * 19 + "11"),
                    "data": "0x",
                })
                return out == bytes(32) and int.from_bytes(out[31:32], "big") == 1
            except Exception:
                return False


if __name__ == "__main__":
    verifier = BLSVerifierOnChain()
    print("EIP-2537 ativo:", verifier.is_eip2537_active())
    # Input de demonstração com comprimento válido (576 bytes de zeros).
    zero_pairing = "0x" + "00" * BLS_PAIRING_INPUT_BYTES
    demo = {
        "proof": {"pairing_input": zero_pairing},
        "public_signals": {"phi": 85},
    }
    print("Resultado:", json.dumps(verifier.verify(demo), indent=2))