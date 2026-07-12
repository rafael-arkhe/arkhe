// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

/// @title PQCVerifier
/// @notice Hybrid classical + post-quantum signature verification for Arkhe accounts.
/// @dev IMPORTANT — read before assuming this verifies ML-DSA on-chain: it does not,
/// and cannot affordably. There is no ML-DSA/ML-KEM precompile in the EVM (the audited
/// prior version of this contract referenced `IMLDSA(MLDSA_PRECOMPILE)` at a fabricated
/// address — no such precompile exists on any EVM chain), and a pure-Solidity lattice
/// signature verifier would cost millions of gas per call, which isn't viable. Ed25519
/// (the classical half `arkhe-crypto-pqc::signature::HybridSigningKey` actually pairs
/// with ML-DSA-65) also has no EVM precompile — so this contract does not pretend to
/// verify Ed25519 on-chain either. It verifies only what the EVM natively supports:
/// secp256k1 ECDSA via `ecrecover`, which is what `msg.sender`/account signing already
/// uses. The PQC half is handled as an off-chain-verified attestation: an authorized
/// oracle runs the real ML-DSA-65 verification (e.g. via `arkhe-crypto-pqc`) and
/// submits an ECDSA-signed attestation over a commitment to the result, which this
/// contract *does* verify on-chain. This is the standard "attestation bridge" pattern
/// used whenever a chain needs a result from computation that isn't affordable inside
/// the EVM itself — it is a bridge to an off-chain PQC check, not a way of verifying
/// ML-DSA on-chain, and is documented as such rather than implied otherwise.
contract PQCVerifier {
    /// @notice Per-account record: the account's classical signer address (recovered
    /// via ECDSA) and a 32-byte keccak256 commitment to its ML-DSA-65 public key. The
    /// full ML-DSA-65 public key (~1952 bytes) is never stored on-chain — only a
    /// commitment to it — versus the classical signer, which needs no separate storage
    /// beyond the recovered `address` (20 bytes), since that's how Ethereum accounts
    /// already work. These two fields are intentionally different types/sizes: the
    /// audited prior version of this contract reused a single `hybridPublicKey` field
    /// for both an Ed25519 key (32 bytes) and an ML-DSA-65 key (2592 bytes), which
    /// can't represent either correctly.
    struct HybridAccount {
        address classicalSigner;
        bytes32 mlDsaPubKeyCommitment;
        bool registered;
    }

    /// @notice The oracle authorized to attest ML-DSA-65 verification results.
    address public immutable authorizedOracle;

    mapping(address => HybridAccount) public accounts;

    event AccountRegistered(address indexed account, address classicalSigner, bytes32 mlDsaPubKeyCommitment);
    event HybridSignatureVerified(address indexed account, bytes32 indexed messageHash);

    error NotRegistered();
    error ClassicalSignatureInvalid();
    error OracleAttestationInvalid();

    constructor(address _authorizedOracle) {
        authorizedOracle = _authorizedOracle;
    }

    /// @notice Registers a hybrid account: binds an on-chain address to its classical
    /// signer and a commitment to its ML-DSA-65 public key.
    function registerAccount(address account, address classicalSigner, bytes32 mlDsaPubKeyCommitment) external {
        accounts[account] =
            HybridAccount({classicalSigner: classicalSigner, mlDsaPubKeyCommitment: mlDsaPubKeyCommitment, registered: true});
        emit AccountRegistered(account, classicalSigner, mlDsaPubKeyCommitment);
    }

    /// @notice Verifies the classical (ECDSA) half of a hybrid signature.
    /// @dev Real, standard `ecrecover`. The audited prior version referenced a
    /// `verifyEd25519` function that was never implemented; this verifies ECDSA
    /// instead, since that's what the EVM can actually check natively — see the
    /// contract-level NatSpec for why Ed25519 isn't verified on-chain either.
    function verifyClassical(bytes32 messageHash, uint8 v, bytes32 r, bytes32 s, address expectedSigner)
        public
        pure
        returns (bool)
    {
        address recovered = ecrecover(messageHash, v, r, s);
        return recovered != address(0) && recovered == expectedSigner;
    }

    /// @notice Verifies a hybrid signature: the classical half on-chain via ECDSA, and
    /// the PQC half via an oracle attestation (also an ECDSA signature, from
    /// `authorizedOracle`) over `keccak256(abi.encode(messageHash, mlDsaPubKeyCommitment))`.
    /// Both must be valid, matching the hybrid posture in `arkhe-crypto-pqc`: breaking
    /// either half alone is not enough to pass this check.
    function verifyHybrid(
        address account,
        bytes32 messageHash,
        uint8 classicalV,
        bytes32 classicalR,
        bytes32 classicalS,
        uint8 oracleV,
        bytes32 oracleR,
        bytes32 oracleS
    ) external returns (bool) {
        HybridAccount memory acct = accounts[account];
        if (!acct.registered) revert NotRegistered();

        if (!verifyClassical(messageHash, classicalV, classicalR, classicalS, acct.classicalSigner)) {
            revert ClassicalSignatureInvalid();
        }

        bytes32 attestationHash = keccak256(abi.encode(messageHash, acct.mlDsaPubKeyCommitment));
        address recoveredOracle = ecrecover(attestationHash, oracleV, oracleR, oracleS);
        if (recoveredOracle != authorizedOracle) {
            revert OracleAttestationInvalid();
        }

        emit HybridSignatureVerified(account, messageHash);
        return true;
    }
}
