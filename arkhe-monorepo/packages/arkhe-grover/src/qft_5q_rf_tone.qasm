OPENQASM 2.0;
include "qelib1.inc";

// ============================================================
// ARKHE - Quantum Fourier Transform (5 Qubits)
// Espectrometro RF quantico - analog do welch/FFT (features.py)
//
// CORRECAO v10.33 (verificada empiricamente com qiskit-aer):
//   v1 ERRADA (motivo duplo):
//     (a) QFT de estado da base |13> gera espectro PLANO (1/32/bin).
//         Delta no tempo = branco na frequencia (uniforme).
//     (b) a decomposicao manual H+cp do v1 nao e a QFT na convencao
//         Qiskit: com rampa ideal ela da pico nos bins errados.
//   CORRETA: a tom RF e a rampa e^{-i 2pi k j / N}; QFT_detector
//            (decomposicao padrao Qiskit) = pico no bin k, como welch.
//   Verificado: 8192/8192 shots no bin |13> (qiskit-aer).
//
// Pipeline: [H + rampa f=-13] (tom RF) -> [QFT] -> pico em |01101>.
// ============================================================

qreg q[5];
creg c[5];
p(pi) q[0];
p(-pi/2) q[0];
h q[0];
p(-pi/2) q[0];
p(3*pi/2) q[0];
p(-pi/2) q[0];
h q[0];
p(-pi/2) q[0];
p(3*pi) q[0];
p(-13*pi/16) q[0];
p(-pi/2) q[0];
h q[0];
p(-pi/2) q[0];
p(pi) q[0];
p(-pi/2) q[0];
h q[0];
p(-pi/2) q[0];
p(3*pi) q[0];
p(pi) q[1];
p(-pi/2) q[1];
h q[1];
p(-pi/2) q[1];
p(3*pi/2) q[1];
p(-pi/2) q[1];
h q[1];
p(-pi/2) q[1];
p(3*pi) q[1];
p(-13*pi/8) q[1];
p(-pi/2) q[1];
h q[1];
p(-pi/2) q[1];
p(pi) q[1];
p(-pi/2) q[1];
h q[1];
p(-pi/2) q[1];
p(3*pi) q[1];
p(pi) q[2];
p(-pi/2) q[2];
h q[2];
p(-pi/2) q[2];
p(3*pi/2) q[2];
p(-pi/2) q[2];
h q[2];
p(-pi/2) q[2];
p(3*pi) q[2];
p(-13*pi/4) q[2];
p(-pi/2) q[2];
h q[2];
p(-pi/2) q[2];
p(pi) q[2];
p(-pi/2) q[2];
h q[2];
p(-pi/2) q[2];
p(3*pi) q[2];
p(pi) q[3];
p(-pi/2) q[3];
h q[3];
p(-pi/2) q[3];
p(3*pi/2) q[3];
p(-pi/2) q[3];
h q[3];
p(-pi/2) q[3];
p(3*pi) q[3];
p(-13*pi/2) q[3];
p(-pi/2) q[3];
h q[3];
p(-pi/2) q[3];
p(pi) q[3];
p(-pi/2) q[3];
h q[3];
p(-pi/2) q[3];
p(3*pi) q[3];
p(pi) q[4];
p(-pi/2) q[4];
h q[4];
p(-pi/2) q[4];
p(3*pi/2) q[4];
p(-pi/2) q[4];
h q[4];
p(-pi/2) q[4];
p(3*pi) q[4];
p(-13*pi) q[4];
p(-pi/2) q[4];
h q[4];
p(-pi/2) q[4];
p(pi) q[4];
p(-pi/2) q[4];
h q[4];
p(-pi/2) q[4];
p(3*pi) q[4];
h q[4];
cp(pi/2) q[4],q[3];
h q[3];
cp(pi/4) q[4],q[2];
cp(pi/2) q[3],q[2];
h q[2];
cp(pi/8) q[4],q[1];
cp(pi/4) q[3],q[1];
cp(pi/2) q[2],q[1];
h q[1];
cp(pi/16) q[4],q[0];
cp(pi/8) q[3],q[0];
cp(pi/4) q[2],q[0];
cp(pi/2) q[1],q[0];
h q[0];
swap q[0],q[4];
swap q[1],q[3];
measure q[0] -> c[0];
measure q[1] -> c[1];
measure q[2] -> c[2];
measure q[3] -> c[3];
measure q[4] -> c[4];