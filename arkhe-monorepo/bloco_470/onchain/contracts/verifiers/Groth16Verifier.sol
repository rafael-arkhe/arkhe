// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.24;

/// @title Pairing — BN254 (alt_bn128) precompile wrappers
/// @notice Implementação clássica usada por verificadores Groth16 (snarkjs).
///         Precompiles: ecadd=0x06, ecmul=0x07, ecpairing=0x08
library Pairing {
    error InvalidPoint();
    error InvalidPairing();

    struct G1Point {
        uint256 X;
        uint256 Y;
    }

    struct G2Point {
        uint256[2] X;
        uint256[2] Y;
    }

    // Prime do corpo base da curva BN254 (alt_bn128)
    uint256 constant Q = 21888242871839275222246405745257275088696311157297823662689037894645226208583;

    function negate(G1Point memory p) internal pure returns (G1Point memory) {
        if (p.X == 0 && p.Y == 0) return G1Point(0, 0);
        return G1Point(p.X, (Q - p.Y) % Q);
    }

    function addition(G1Point memory p1, G1Point memory p2) internal view returns (G1Point memory r) {
        uint256[4] memory input;
        input[0] = p1.X;
        input[1] = p1.Y;
        input[2] = p2.X;
        input[3] = p2.Y;
        bool success;
        // solhint-disable-next-line no-inline-assembly
        assembly {
            success := staticcall(sub(gas(), 2000), 6, input, 0x80, r, 0x40)
        }
        if (!success) revert InvalidPoint();
    }

    function scalar_mul(G1Point memory p, uint256 s) internal view returns (G1Point memory r) {
        uint256[3] memory input;
        input[0] = p.X;
        input[1] = p.Y;
        input[2] = s;
        bool success;
        // solhint-disable-next-line no-inline-assembly
        assembly {
            success := staticcall(sub(gas(), 2000), 7, input, 0x60, r, 0x40)
        }
        if (!success) revert InvalidPoint();
    }

    /// @dev Produto de 4 pareamentos: e(a1,a2) * e(b1,b2) * e(c1,c2) * e(d1,d2) == 1
    ///      (formato Groth16). Input: 4 pares x 192 bytes = 768 bytes (0x300).
    function pairingProd4(
        G1Point memory a1, G2Point memory a2,
        G1Point memory b1, G2Point memory b2,
        G1Point memory c1, G2Point memory c2,
        G1Point memory d1, G2Point memory d2
    ) internal view returns (bool) {
        uint256[24] memory input;
        // Cada par: G1 ocupa 2 palavras; G2 ocupa 4 palavras.
        _copyG1(input, 0, a1);
        _copyG2(input, 2, a2);
        _copyG1(input, 6, b1);
        _copyG2(input, 8, b2);
        _copyG1(input, 12, c1);
        _copyG2(input, 14, c2);
        _copyG1(input, 18, d1);
        _copyG2(input, 20, d2);

        uint256[1] memory out;
        bool success;
        // solhint-disable-next-line no-inline-assembly
        assembly {
            success := staticcall(sub(gas(), 2000), 8, input, 0x300, out, 0x20)
        }
        if (!success) revert InvalidPairing();
        return out[0] != 0;
    }

    function _copyG1(uint256[24] memory input, uint256 offset, G1Point memory p) internal pure {
        input[offset] = p.X;
        input[offset + 1] = p.Y;
    }

    function _copyG2(uint256[24] memory input, uint256 offset, G2Point memory p) internal pure {
        input[offset] = p.X[0];
        input[offset + 1] = p.X[1];
        input[offset + 2] = p.Y[0];
        input[offset + 3] = p.Y[1];
    }
}

/// @title Groth16 verifier para circuitos de governança da Catedral OS
/// @notice Verifica provas zk-SNARK (Groth16, curva BN254).
/// @dev A chave de verificação abaixo é um PLACEHOLDER do snarkjs. Em produção:
///      snarkjs zkey export solidityverifier <zkey> Verifier.sol
contract Groth16Verifier {
    using Pairing for *;

    struct VerifyingKey {
        Pairing.G1Point alpha1;
        Pairing.G2Point beta2;
        Pairing.G2Point gamma2;
        Pairing.G2Point delta2;
        Pairing.G1Point[] ic;
    }

    struct Proof {
        Pairing.G1Point a;
        Pairing.G2Point b;
        Pairing.G1Point c;
    }

    function verifyingKey() internal pure returns (VerifyingKey memory vk) {
        vk.alpha1 = Pairing.G1Point(
            20491192805390485299153009773594534940189261866228447918068658471970481763042,
            9383485363053290200918347156157836566562967994039712273449902621266178545958
        );
        vk.beta2 = Pairing.G2Point(
            [
                4252822878758300859123897981450591353533073413197771765191448309293591100739,
                1768004226013873187218005289769154510208963026236515868815532290158335863353
            ],
            [
                20453052444310952364478961628758313643713960264077193584124160312538473244537,
                19824756730678996460835493530299078984855514540732907174198919500176316649042
            ]
        );
        vk.gamma2 = Pairing.G2Point(
            [
                1768004226013873187218005289769154510208963026236515868815532290158335863353,
                19824756730678996460835493530299078984855514540732907174198919500176316649042
            ],
            [
                14272186543608359836601386059294055710192607309889680204674073601828530623626,
                11951645084210376741593100827948330981517466974130563615257295438335354635257
            ]
        );
        vk.delta2 = Pairing.G2Point(
            [
                17430273546676730069488018824546249222439185436222272476132348635242263020594,
                20988953248784869334815953895545265427963566994623956586262773003249601258669
            ],
            [
                15230842267856027161808732059556714866893112747897925190161762561809929712720,
                196408249106486839345020555991847385013520127140404714147214377666022225842
            ]
        );
        vk.ic = new Pairing.G1Point[](2);
        vk.ic[0] = Pairing.G1Point(
            3528090205984778953540498605982954428935277551801369440294743708266998685084,
            13631677540434299764102816753554477841206009567144303951210274883870558256280
        );
        vk.ic[1] = Pairing.G1Point(
            14189180196614138110233760479035422851824881184575117958131171487004003882197,
            3666543923155564634237419597517555244372764115478370178939226997920600263398
        );
    }

    function verifyProof(
        uint256[2] memory a,
        uint256[2][2] memory b,
        uint256[2] memory c,
        uint256[1] memory input
    ) public view returns (bool) {
        Proof memory proof;
        proof.a = Pairing.G1Point(a[0], a[1]);
        proof.b = Pairing.G2Point([b[0][0], b[0][1]], [b[1][0], b[1][1]]);
        proof.c = Pairing.G1Point(c[0], c[1]);

        VerifyingKey memory vk = verifyingKey();

        // input + 1 deve casar com o número de points ic
        if (vk.ic.length != input.length + 1) return false;

        // vk_x = ic[0] + ic[1] * input[0] + ...
        Pairing.G1Point memory vk_x = Pairing.G1Point(0, 0);
        for (uint256 i = 0; i < input.length; i++) {
            if (input[i] >= Pairing.Q) return false;
            vk_x = Pairing.addition(vk_x, Pairing.scalar_mul(vk.ic[i + 1], input[i]));
        }
        vk_x = Pairing.addition(vk_x, vk.ic[0]);

        // e(pi_a, pi_b) * e(-vk_x, gamma2) * e(-pi_c, delta2) * e(-alpha1, beta2) == 1
        if (!Pairing.pairingProd4(
            proof.a, proof.b,
            Pairing.negate(vk_x), vk.gamma2,
            Pairing.negate(proof.c), vk.delta2,
            Pairing.negate(vk.alpha1), vk.beta2
        )) return false;

        return true;
    }
}

/// @title Verificador de Governança (v13, O1)
contract GovernanceVerifier is Groth16Verifier {
    event Verified(address indexed sender, bool success, uint256 phi);
    event ContractDeployed(address indexed deployer, string circuit, string curve);

    uint256 public constant THRESHOLD = 85; // Φ ≥ 85%

    constructor() {
        emit ContractDeployed(msg.sender, "governance_consensus", "BN254");
    }

    /// @dev Verifica uma prova de governança e emite evento.
    function verifyGovernance(
        uint256[2] memory a,
        uint256[2][2] memory b,
        uint256[2] memory c,
        uint256[1] memory input
    ) public returns (bool) {
        bool ok = verifyProof(a, b, c, input);
        emit Verified(msg.sender, ok, input[0]);
        return ok;
    }

    /// @dev Verifica prova com validação adicional de threshold.
    function verifyGovernanceWithThreshold(
        uint256[2] memory a,
        uint256[2][2] memory b,
        uint256[2] memory c,
        uint256[1] memory input,
        uint256 threshold
    ) public view returns (bool) {
        require(threshold <= 100, "Threshold must be <= 100");
        bool ok = verifyProof(a, b, c, input);
        return ok && input[0] >= threshold;
    }
}