// SPDX-License-Identifier: MIT
pragma solidity ^0.8.18;

import {GeneralRandcastConsumerBase, BasicRandcastConsumerBase} from "randcast-user-contract/user/GeneralRandcastConsumerBase.sol";

/// @title CatedralRandcastConsumer
/// @notice Consumidor de aleatoriedade verificável (ARPA Randcast / BLS-TSS).
/// @dev BLOCO 484 v45 — Catedral OS.
///      Interface verificada contra o repositório oficial
///      ARPA-Network/Randcast-User-Contract (GeneralRandcastConsumerBase).
///      - `_fulfillRandomness(bytes32, uint256)` (aleatoriedade é uint256).
///      - `_requestRandomness(RequestType, bytes)` retorna bytes32 requestId.
///      - Solicitação restrita a `onlyOwner` (docs ARPA: WARNING — sem guarda,
///        qualquer EOA drena a assinatura/subscription).
///      Compila com: solc ^0.8.18 --import-remappings "randcast-user-contract/=<root>/contracts/"
contract CatedralRandcastConsumer is GeneralRandcastConsumerBase {
    /// @notice Aleatoriedade entregue pelo Adapter para um requestId.
    event RandomnessRequested(bytes32 indexed requestId, address indexed requester);

    /// @notice Fulfillment com o valor sorteador pelo grupo BLS-TSS.
    event RandomnessFulfilled(bytes32 indexed requestId, uint256 randomness, uint256 timestamp);

    /// @notice Callback para o endpoint falhou (aleatoriedade continua no ledger).
    event RandomnessCallbackFailed(bytes32 indexed requestId);

    /// @notice requestId -> endpoint (contrato ou EOA) que recebe a aleatoriedade.
    mapping(bytes32 => address) public requestCallbacks;

    constructor(address adapter) BasicRandcastConsumerBase(adapter) {}

    /// @notice Solicita aleatoriedade verificável à ARPA Network.
    /// @dev Gate `onlyOwner`: protege a assinatura de gasto por terceiros.
    function requestRandomness(address callbackAddress)
        external
        onlyOwner
        returns (bytes32 requestId)
    {
        bytes memory params;
        requestId = _requestRandomness(RequestType.Randomness, params);
        requestCallbacks[requestId] = callbackAddress;
        emit RandomnessRequested(requestId, msg.sender);
        return requestId;
    }

    /// @notice Callback invocado pelo Adapter quando a aleatoriedade chega.
    /// @dev A aleatoriedade já é publicada no evento (LEDGER on-chain); o
    ///      callback é best-effort e nunca reverte o fulfillment.
    function _fulfillRandomness(bytes32 requestId, uint256 randomness) internal override {
        emit RandomnessFulfilled(requestId, randomness, block.timestamp);

        address callback = requestCallbacks[requestId];
        if (callback == address(0)) return;
        // best-effort: falha no callback não reverte (aleatoriedade já emitida)
        (bool ok, ) = callback.call(
            abi.encodeWithSignature(
                "onRandomnessReceived(bytes32,uint256)",
                requestId,
                randomness
            )
        );
        if (!ok) emit RandomnessCallbackFailed(requestId);
    }
}