"""
crypto_utils.py — Criptografia AES-256-GCM para dados sensíveis.
"""
import os
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.backends import default_backend
import base64
from typing import Tuple


def generate_key_from_password(password: str, salt: bytes = None) -> Tuple[bytes, bytes]:
    """
    Deriva uma chave AES-256 a partir de uma senha usando PBKDF2.
    Retorna (key, salt).
    """
    if salt is None:
        salt = os.urandom(16)
    kdf = PBKDF2HMAC(
        algorithm=hashes.SHA256(),
        length=32,
        salt=salt,
        iterations=100000,
        backend=default_backend()
    )
    key = kdf.derive(password.encode('utf-8'))
    return key, salt


def encrypt_aes_gcm(plaintext: bytes, key: bytes) -> Tuple[bytes, bytes]:
    """
    Criptografa dados com AES-256-GCM.
    Retorna (nonce, ciphertext). O tag de autenticação (16 bytes) é
    anexado ao ciphertext pelo backend (pyca/cryptography).
    """
    aesgcm = AESGCM(key)
    nonce = os.urandom(12)
    ciphertext = aesgcm.encrypt(nonce, plaintext, None)
    return nonce, ciphertext


def decrypt_aes_gcm(ciphertext: bytes, key: bytes, nonce: bytes) -> bytes:
    """Decriptografa dados com AES-256-GCM e verifica a autenticidade."""
    aesgcm = AESGCM(key)
    return aesgcm.decrypt(nonce, ciphertext, None)


# Exemplo de uso
if __name__ == "__main__":
    # Deriva chave de uma senha
    password = "minha_senha_secreta"
    key, salt = generate_key_from_password(password)

    # Criptografa
    plaintext = "Dados sensíveis da Catedral OS".encode("utf-8")
    nonce, ciphertext = encrypt_aes_gcm(plaintext, key)

    # Decriptografa
    decrypted = decrypt_aes_gcm(ciphertext, key, nonce)
    assert decrypted == plaintext
    print("AES-256-GCM funcionando corretamente.")
