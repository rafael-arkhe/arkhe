// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.24;

/// @title BLS12-381 Verifier (v15, B1) — EIP-2537 (Pectra)
/// @notice Verificador BLS12-381 on-chain que encaminha o input de pareamento
///         (EIP-2537, precompile 0x11) já codificado pela biblioteca off-chain.
///
/// @dev POR QUE O INPUT É "OPAQUE":
///       Os elementos de campo de BLS12-381 têm 381 bits e NÃO cabem em uint256.
///       Solidity não possui bytes48 (apenas bytes1..bytes32). Por isso, em
///       produção (mesma abordagem de projetos como Polygon, Iden3 e contratos
///       de staking ETH2), as coordenadas são codificadas off-chain como 48
///       bytes little-endian e o contrato apenas valida o comprimento e
///       repassa à precompile de pareamento. O input completo (dois pares)
///       é montado por bls_verifier_v15.py (ou libs: blst/py_ecc/noble).
///
///      Formato do input (2 pares = 576 bytes):
///        par 1: e(proof.a, pubkey)  → G1(96B) ∥ G2(192B)
///        par 2: e(proof.c, -g2)     → G1(96B) ∥ G2(192B)
///
///      Requer rede com fork Pectra (precompiles 0x0b..0x13 ativas).
contract BLS12_381Verifier {
    address constant BLS12_PAIRING = address(0x11);

    event Verified(address indexed sender, bool success, bytes32 publicInput);
    event ContractDeployed(address indexed deployer, string curve, string precompiles);

    constructor() {
        emit ContractDeployed(msg.sender, "BLS12-381", "EIP-2537 (0x0b-0x13)");
    }

    /// @dev Valida o comprimento e encaminha o input à precompile de pareamento (0x11).
    function _pairingCheck(bytes calldata pairingInput) private view returns (bool) {
        // EIP-2537: o input deve ser múltiplo de 192 bytes (1 par = 6 elementos de 48 B).
        if (pairingInput.length == 0 || pairingInput.length % 192 != 0) return false;
        (bool success, bytes memory out) = BLS12_PAIRING.staticcall(pairingInput);
        return success && out.length == 32 && abi.decode(out, (uint256)) == 1;
    }

    /// @dev Verifica prova BLS12-381. `pairingInput` = input EIP-2537 pronto (ver docstring).
    function verify(bytes calldata pairingInput, bytes32[1] memory publicInput)
        external
        returns (bool)
    {
        bool ok = _pairingCheck(pairingInput);
        emit Verified(msg.sender, ok, publicInput[0]);
        return ok;
    }

    /// @dev Checa se a precompile de pareamento está ativa no fork atual.
    ///      Chamada com input vazio: produto vazio = 1 (retorna 32 bytes) quando ativo;
    ///      endereço sem código retorna success com returndata vazio.
    function isEip2537Active() public view returns (bool) {
        (bool success, bytes memory out) = BLS12_PAIRING.staticcall("");
        return success && out.length == 32 && abi.decode(out, (uint256)) == 1;
    }
}