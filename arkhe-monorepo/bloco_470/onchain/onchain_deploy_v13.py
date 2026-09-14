# =============================================================================
# BLOCO 470 v13 — DEPLOY ON-CHAIN (O1)
# onchain_deploy_v13.py
#
# Deploy e verificação de contratos de governança em Sepolia.
#
# Requer: pip install web3 eth-account requests
# Env:    PRIVATE_KEY, ETHERSCAN_API_KEY, SEPOLIA_RPC_URL
# Uso:    python onchain_deploy_v13.py  (deploy)
#         python onchain_deploy_v13.py --verify <endereco>
# =============================================================================
import json
import os
from web3 import Web3
from eth_account import Account
from pathlib import Path
import requests
import time
from typing import Dict, Optional, List


class OnChainDeployer:
    """Gerencia deploy e verificação de contratos em Sepolia."""

    CHAIN_ID = 11155111  # Sepolia

    def __init__(self, rpc_url: str = None):
        self.rpc_url = rpc_url or os.environ.get("SEPOLIA_RPC_URL", "https://rpc.sepolia.org")
        self.w3 = Web3(Web3.HTTPProvider(self.rpc_url))
        if not self.w3.is_connected():
            raise ConnectionError(f"Não foi possível conectar a {self.rpc_url}")

    # ------------------------------------------------------------------------
    # DEPLOY
    # ------------------------------------------------------------------------
    def deploy_verifier(self, private_key: str, contract_json_path: str) -> Dict:
        """
        Deploy do verificador Groth16 em Sepolia a partir do artefato JSON
        (artifacts/contracts/verifiers/Groth16Verifier.sol/GovernanceVerifier.json).
        """
        account = Account.from_key(private_key)

        with open(contract_json_path) as f:
            contract_json = json.load(f)

        abi = contract_json["abi"]
        bytecode = contract_json["bytecode"]

        contract = self.w3.eth.contract(abi=abi, bytecode=bytecode)
        nonce = self.w3.eth.get_transaction_count(account.address)

        tx = contract.constructor().build_transaction({
            "from": account.address,
            "nonce": nonce,
            "gas": 3_000_000,
            "gasPrice": self.w3.eth.gas_price,
            "chainId": self.CHAIN_ID,
        })

        signed_tx = account.sign_transaction(tx)
        tx_hash = self.w3.eth.send_raw_transaction(signed_tx.rawTransaction)

        receipt = self.w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
        print(f"✅ Verifier deployed at: {receipt.contractAddress} (block {receipt.blockNumber})")

        # Verificação no Etherscan
        self._verify_on_etherscan(receipt.contractAddress, contract_json_path, abi, contract_json)

        return {
            "address": receipt.contractAddress,
            "tx_hash": tx_hash.hex(),
            "block": receipt.blockNumber,
            "network": "sepolia",
            "curve": "BN254",
            "contract": "GovernanceVerifier",
        }

    # ------------------------------------------------------------------------
    # VERIFICAÇÃO DE PROVA GROTH16
    # ------------------------------------------------------------------------
    def verify_proof_onchain(self, verifier_address: str, proof: Dict) -> bool:
        """
        Verifica uma prova zk-SNARK on-chain no GovernanceVerifier.
        proof precisa das chaves: a (2), b (2x2), c (2), public_signals (list).
        """
        abi = self.governance_abi()
        contract = self.w3.eth.contract(address=verifier_address, abi=abi)

        a = proof.get("a")
        b = proof.get("b")
        c = proof.get("c")
        inputs = [int(x) for x in proof.get("public_signals", [])]

        # Uso do formato do evento / retorno do contrato
        try:
            ok = contract.functions.verifyGovernance(a, b, c, inputs).call()
            return bool(ok)
        except Exception as e:
            print(f"Verification failed: {e}")
            return False

    @staticmethod
    def governance_abi() -> List[dict]:
        """ABI mínima do GovernanceVerifier (event + funções de verificação)."""
        return [
            {
                "anonymous": False,
                "inputs": [
                    {"indexed": True, "internalType": "address", "name": "sender", "type": "address"},
                    {"indexed": False, "internalType": "bool", "name": "success", "type": "bool"},
                    {"indexed": False, "internalType": "uint256", "name": "phi", "type": "uint256"},
                ],
                "name": "Verified",
                "type": "event",
            },
            {
                "inputs": [
                    {"internalType": "uint256[2]", "name": "a", "type": "uint256[2]"},
                    {"internalType": "uint256[2][2]", "name": "b", "type": "uint256[2][2]"},
                    {"internalType": "uint256[2]", "name": "c", "type": "uint256[2]"},
                    {"internalType": "uint256[1]", "name": "input", "type": "uint256[1]"},
                ],
                "name": "verifyGovernance",
                "outputs": [{"internalType": "bool", "name": "", "type": "bool"}],
                "stateMutability": "nonpayable",
                "type": "function",
            },
            {
                "inputs": [
                    {"internalType": "uint256[2]", "name": "a", "type": "uint256[2]"},
                    {"internalType": "uint256[2][2]", "name": "b", "type": "uint256[2][2]"},
                    {"internalType": "uint256[2]", "name": "c", "type": "uint256[2]"},
                    {"internalType": "uint256[1]", "name": "input", "type": "uint256[1]"},
                ],
                "name": "verifyProof",
                "outputs": [{"internalType": "bool", "name": "", "type": "bool"}],
                "stateMutability": "view",
                "type": "function",
            },
        ]

    # ------------------------------------------------------------------------
    # ETHERSCAN
    # ------------------------------------------------------------------------
    def _verify_on_etherscan(self, address: str, source_path: str, abi, contract_json: dict) -> None:
        api_key = os.environ.get("ETHERSCAN_API_KEY")
        if not api_key:
            print("⚠️ ETHERSCAN_API_KEY não definida; pulando verificação.")
            return

        try:
            source_code = contract_json.get("metadata", {}).get("sources", {})
        except Exception:
            source_code = {}

        # Em produção, guarde o fonte completo no artefato; aqui usamos o arquivo .sol
        source = contract_json.get("sourceCode")
        if not source:
            try:
                source = Path(source_path).read_text()
            except Exception:
                source = "contract generated by snarkjs; see repo bloco_470/onchain"

        params = {
            "module": "contract",
            "action": "verifysourcecode",
            "apikey": api_key,
            "contractaddress": address,
            "sourceCode": json.dumps({"language": "Solidity", "sources": {"Groth16Verifier.sol": {"content": source}}, "settings": {"optimizer": {"enabled": True, "runs": 200}}}),
            "codeformat": "solidity-standard-json-input",
            "contractname": "contracts/verifiers/Groth16Verifier.sol:GovernanceVerifier",
            "compilerversion": "v0.8.24",
            "evmversion": "paris",
        }

        r = requests.post("https://api-sepolia.etherscan.io/api", data=params, timeout=30)
        if r.status_code == 200:
            print(f"✅ Verificação submetida ao Etherscan: {r.json().get('message')}")
        else:
            print(f"⚠️ Falha ao submeter verificação: {r.text[:200]}")


if __name__ == "__main__":
    import sys

    deployer = OnChainDeployer()

    if len(sys.argv) > 1 and sys.argv[1] == "--verify":
        addr = sys.argv[2]
        proof = {
            "a": [0, 0],
            "b": [[0, 0], [0, 0]],
            "c": [0, 0],
            "public_signals": [85],
        }
        print("Resultado on-chain:", deployer.verify_proof_onchain(addr, proof))
    else:
        pk = os.environ.get("PRIVATE_KEY")
        if not pk:
            raise SystemExit("PRIVATE_KEY é obrigatória.")
        artifact = os.environ.get("VERIFIER_ARTIFACT", "artifacts/contracts/verifiers/Groth16Verifier.sol/GovernanceVerifier.json")
        result = deployer.deploy_verifier(pk, artifact)
        print(json.dumps(result, indent=2))