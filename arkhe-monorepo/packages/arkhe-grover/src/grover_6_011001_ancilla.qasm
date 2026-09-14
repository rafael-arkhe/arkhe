// ============================================================
// 6-Qubit Grover Search — Marked State |011001>
// Qiskit bit order: q[0]=LSB → |011001> means
//   q0=1, q1=0, q2=0, q3=1, q4=1, q5=0
// Optimal iterations: floor(pi/4 * sqrt(64)) = 6
//
// Uses 3 ancilla qubits (a0-a2) to decompose each 5-CX
// into 8 x ccx gates (Toffoli) - no phase ambiguity.
// Total: 9 qubits, 6 data + 3 ancilla (always returned to |0>)
// ============================================================

OPENQASM 2.0;
include "qelib1.inc";

qreg q[6];     // data qubits
qreg a[3];     // ancilla qubits
creg c[6];

gate c5x q0, q1, q2, q3, q4, t, a0, a1, a2 {
  ccx q0, q1, a0;
  ccx q2, q3, a1;
  ccx a0, a1, a2;
  ccx a2, q4, t;
  ccx a0, a1, a2;
  ccx q2, q3, a1;
  ccx q0, q1, a0;
}

gate oracle q0, q1, q2, q3, q4, q5, a0, a1, a2 {
  x q1;  x q2;  x q5;
  h q5;
  c5x q0, q1, q2, q3, q4, q5, a0, a1, a2;
  h q5;
  x q1;  x q2;  x q5;
}

gate diffuser q0, q1, q2, q3, q4, q5, a0, a1, a2 {
  h q0; h q1; h q2; h q3; h q4; h q5;
  x q0; x q1; x q2; x q3; x q4; x q5;
  h q5;
  c5x q0, q1, q2, q3, q4, q5, a0, a1, a2;
  h q5;
  x q0; x q1; x q2; x q3; x q4; x q5;
  h q0; h q1; h q2; h q3; h q4; h q5;
}

h q[0]; h q[1]; h q[2]; h q[3]; h q[4]; h q[5];

oracle  q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];
diffuser q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];

oracle  q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];
diffuser q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];

oracle  q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];
diffuser q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];

oracle  q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];
diffuser q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];

oracle  q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];
diffuser q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];

oracle  q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];
diffuser q[0], q[1], q[2], q[3], q[4], q[5], a[0], a[1], a[2];

measure q[0] -> c[0];
measure q[1] -> c[1];
measure q[2] -> c[2];
measure q[3] -> c[3];
measure q[4] -> c[4];
measure q[5] -> c[5];