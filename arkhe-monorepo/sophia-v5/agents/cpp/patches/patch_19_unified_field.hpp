#pragma once
// Patch #19 — Unified Coherence Field (ψ_C)
// Gap-1 Constitution: 0.577350 < Φ_C <= 0.999900
namespace Sophia::Local {

class UnifiedCoherenceField {
public:
    static constexpr double kPhiMin = 0.577350;
    static constexpr double kPhiMax = 0.999900;

    explicit UnifiedCoherenceField(double psi = kPhiMin, double jitter = 0.01)
        : psi_(clamp(psi, kPhiMin, kPhiMax)), jitter_(jitter) {}

    double psi_C() const { return psi_; }

    void setPsi_C(double value) { psi_ = clamp(value, kPhiMin, kPhiMax); }

    // Avança um passo com perturbação amortecida; retorna novo ψ_C.
    double step(double random_in_01 = 0.5) {
        double delta = jitter_ * (random_in_01 - 0.5);
        setPsi_C(psi_ + delta);
        return psi_;
    }

    bool inBounds() const { return psi_ >= kPhiMin && psi_ <= kPhiMax; }

private:
    static double clamp(double v, double lo, double hi) {
        return v < lo ? lo : (v > hi ? hi : v);
    }
    double psi_;
    double jitter_;
};

} // namespace Sophia::Local