#pragma once
// Patch #20 — Quantum Entanglement Coefficient (Q), a partir de estudo twin local.
#include <vector>
#include <cmath>
#include <cstddef>

namespace Sophia::Local {

class QuantumEntanglementCoefficient {
public:
    double qValue() const { return q_; }
    std::size_t trials() const { return trials_; }

    // Mede Q a partir de uma matriz de respostas (estudo twin).
    void measureFromTwinStudy(const std::vector<double>& responses) {
        if (responses.empty()) return;
        double sum = 0.0;
        for (double r : responses) sum += r;
        double mean = sum / static_cast<double>(responses.size());
        q_ = clamp(std::fabs(std::tanh(mean * 4.0)), 0.0, 1.0);
        trials_ = responses.size();
    }

private:
    static double clamp(double v, double lo, double hi) {
        return v < lo ? lo : (v > hi ? hi : v);
    }
    double q_ = 0.0;
    std::size_t trials_ = 0;
};

} // namespace Sophia::Local