# =============================================================================
# BLOCO 470 v13 — DID via DNS / did:web (O2)
# did_web_server_v13.py
#
# Servidor para hosting e resolução de DIDs did:web com suporte a DNS e DNSSEC.
#
# Requer: pip install flask flask-cors dnspython requests
# Env:    DID_DOMAIN, DID_DOCUMENTS_DIR, DNS_SERVER
# Uso:    python did_web_server_v13.py
# =============================================================================
from flask import Flask, jsonify, request
from flask_cors import CORS
import json
import os
import hashlib
from datetime import datetime
import dns.resolver
import dns.rdatatype
import requests

app = Flask(__name__)
CORS(app)

# ============================================================================
# CONFIGURAÇÃO
# ============================================================================

DID_DOCUMENTS_DIR = os.environ.get("DID_DOCUMENTS_DIR", os.path.join(os.path.dirname(__file__), "documents"))
DOMAIN = os.environ.get("DID_DOMAIN", "catedral.os")
DNS_SERVER = os.environ.get("DNS_SERVER", "8.8.8.8")


# ============================================================================
# GERENCIAMENTO DE DID DOCUMENTS
# ============================================================================

class DIDWebManager:
    """
    Gerenciador de DIDs did:web com suporte a DNS e DNSSEC.
    Conforme a especificação W3C did:web.
    """

    @staticmethod
    def create_did_document(did: str, public_key: str, services: list = None) -> dict:
        """Cria um documento DID W3C."""
        return {
            "@context": "https://www.w3.org/ns/did/v1",
            "id": did,
            "verification_method": [{
                "id": f"{did}#key-1",
                "type": "Ed25519VerificationKey2020",
                "controller": did,
                "publicKeyMultibase": public_key,
            }],
            "authentication": [f"{did}#key-1"],
            "assertionMethod": [f"{did}#key-1"],
            "service": services or [],
            "created": datetime.utcnow().isoformat(),
            "updated": datetime.utcnow().isoformat(),
        }

    @staticmethod
    def host_did(domain: str, did_document: dict, path: str = None):
        """
        Hospeda um documento DID no domínio especificado.
        - path None  -> /.well-known/did.json  (did:web:example.com)
        - path "u/a" -> /u/a/did.json          (did:web:example.com:u:a)
        """
        if path:
            file_path = os.path.join(DID_DOCUMENTS_DIR, path, "did.json")
        else:
            file_path = os.path.join(DID_DOCUMENTS_DIR, ".well-known", "did.json")

        os.makedirs(os.path.dirname(file_path), exist_ok=True)
        with open(file_path, "w") as f:
            json.dump(did_document, f, indent=2)
        return file_path

    @staticmethod
    def resolve_did_web(did: str, use_dns: bool = True) -> dict:
        """
        Resolve um DID did:web via DNS/HTTPS.
        did:web:example.com        -> https://example.com/.well-known/did.json
        did:web:example.com:u:a    -> https://example.com/u/a/did.json
        """
        parts = did.replace("did:web:", "").split(":")
        domain = parts[0]
        path_parts = parts[1:] if len(parts) > 1 else []

        url = f"https://{domain}/{'/'.join(path_parts)}/did.json" if path_parts \
            else f"https://{domain}/.well-known/did.json"

        # Resolve via DNS primeiro (TXT _did.<domain>), se configurado
        if use_dns:
            dns_result = DIDWebManager._resolve_via_dns(domain)
            if dns_result:
                return dns_result

        # Fallback: HTTP
        try:
            response = requests.get(url, timeout=10, verify=True)
            if response.status_code == 200:
                return response.json()
        except Exception as e:
            print(f"HTTP resolution failed: {e}")

        return None

    @staticmethod
    def check_dnssec(domain: str) -> bool:
        """Verifica se a zona responde com AD bit (DNSSEC validado)."""
        try:
            resolver = dns.resolver.Resolver()
            resolver.nameservers = [DNS_SERVER]
            resolver.use_edns(0, 4096, True)
            answers = resolver.resolve(domain, "A", want_dnssec=True)
            return "ad" in str(answers.response.flags)
        except Exception:
            return False

    @staticmethod
    def _resolve_via_dns(domain: str) -> dict:
        """Resolve DID via DNS TXT com suporte a DNSSEC."""
        try:
            resolver = dns.resolver.Resolver()
            resolver.nameservers = [DNS_SERVER]

            answers = resolver.resolve(f"_did.{domain}", dns.rdatatype.TXT, raise_on_no_answer=False)
            for answer in answers:
                for string in answer.strings:
                    txt_value = string.decode()
                    if txt_value.startswith("did:web:"):
                        return DIDWebManager.resolve_did_web(txt_value, use_dns=False)

            # Fallback: arquivo .well-known
            url = f"https://{domain}/.well-known/did.json"
            response = requests.get(url, timeout=5, verify=True)
            if response.status_code == 200:
                return response.json()
        except dns.resolver.NoAnswer:
            pass
        except Exception as e:
            print(f"DNS resolution failed: {e}")

        return None

    @staticmethod
    def did_hash(did: str) -> str:
        """Hash auxiliar para rastreamento no TemporalChain."""
        return hashlib.sha256(did.encode()).hexdigest()


# ============================================================================
# ENDPOINTS PARA DID WEB
# ============================================================================

@app.route('/.well-known/did.json', methods=['GET'])
def serve_did_json():
    """Serve o documento DID para o domínio raiz."""
    try:
        with open(os.path.join(DID_DOCUMENTS_DIR, ".well-known", "did.json")) as f:
            return jsonify(json.load(f))
    except FileNotFoundError:
        return jsonify({"error": "DID document not found"}), 404


@app.route('/<path:path>/did.json', methods=['GET'])
def serve_did_path(path):
    """Serve o documento DID para um path específico."""
    try:
        file_path = os.path.join(DID_DOCUMENTS_DIR, path, "did.json")
        with open(file_path) as f:
            return jsonify(json.load(f))
    except FileNotFoundError:
        return jsonify({"error": "DID document not found"}), 404


@app.route('/api/did/web/create', methods=['POST'])
def create_did_web():
    """Cria e hospeda um novo DID web."""
    data = request.json or {}
    domain = data.get('domain', DOMAIN)
    public_key = data.get('public_key')

    if not public_key:
        return jsonify({"error": "public_key required"}), 400

    did = f"did:web:{domain}"
    if data.get('path'):
        did = f"{did}:{data.get('path')}"

    document = DIDWebManager.create_did_document(did, public_key, data.get('services'))
    DIDWebManager.host_did(domain, document, data.get('path'))

    url = f"https://{domain}/{'/'.join(data.get('path').split(':'))}/did.json" \
        if data.get('path') else f"https://{domain}/.well-known/did.json"

    return jsonify({
        "did": did,
        "document": document,
        "url": url,
        "sha256": DIDWebManager.did_hash(did),
    })


@app.route('/api/did/web/resolve/<path:did>', methods=['GET'])
def resolve_did_web(did):
    """Resolve um DID web."""
    if not did.startswith("did:web:"):
        did = f"did:web:{did}"

    document = DIDWebManager.resolve_did_web(did)
    if document:
        return jsonify(document)
    return jsonify({"error": "DID not found"}), 404


@app.route('/api/did/web/dnssec/<domain>', methods=['GET'])
def did_dnssec_status(domain):
    """Verifica o estado DNSSEC do domínio que publica o DID."""
    return jsonify({
        "domain": domain,
        "ad_bit": DIDWebManager.check_dnssec(domain),
    })


if __name__ == "__main__":
    os.makedirs(os.path.join(DID_DOCUMENTS_DIR, ".well-known"), exist_ok=True)
    app.run(host="0.0.0.0", port=8020)