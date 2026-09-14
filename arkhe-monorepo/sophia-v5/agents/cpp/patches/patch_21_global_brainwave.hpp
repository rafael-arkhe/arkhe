#pragma once
// Patch #21 — Global Brainwave Network (coerência global entre agentes).
namespace Sophia::Local {

class GlobalBrainwaveNetwork {
public:
    explicit GlobalBrainwaveNetwork(double coherence = 0.5, double jitter = 0.02)
        : coherence_(clamp(coherence, 0.0, 1.0)), jitter_(jitter) {}

    double globalCoherence() const { return coherence_; }

    void setGlobalCoherence(double value) { coherence_ = clamp(value, 0.0, 1.0); }

    double step(double random_in_01 = 0.5) {
        double delta = jitter_ * (random_in_01 - 0.5);
        setGlobalCoherence(coherence_ + delta);
        return coherence_;
    }

private:
    static double clamp(double v, double lo, double hi) {
        return v < lo ? lo : (v > hi ? hi : v);
    }
    double coherence_;
    double jitter_;
};

} // namespace Sophia::Local