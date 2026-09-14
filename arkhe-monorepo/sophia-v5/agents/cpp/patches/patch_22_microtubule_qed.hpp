#pragma once
// Patch #22 — Microtubule QED Cavity (dinâmica de cavidade, decoerência).
#include <vector>
#include <cmath>
#include <cstddef>

namespace Sophia::Local {

class MicrotubuleQEDCavity {
public:
    explicit MicrotubuleQEDCavity(double kappa = 1e5, double temperature = 5.0)
        : kappa_(kappa), temperature_(temperature) {}

    void setKappa(double kappa) { kappa_ = kappa; }
    double decoherenceTime() const { return 1.0 / kappa_; }

    // Amplitude de excitação E(t) = exp(-κt) + ruído térmico.
    std::vector<double> simulateCavityDynamics(double time_s, int steps) const {
        std::vector<double> out;
        out.reserve(static_cast<std::size_t>(steps));
        const double dt = time_s / static_cast<double>(steps > 0 ? steps : 1);
        for (int i = 0; i < steps; ++i) {
            const double t = dt * static_cast<double>(i);
            const double thermal =
                std::sqrt(temperature_) * 0.05 * std::sin(2.0 * 3.141592653589793 * t * 1e6);
            out.push_back(std::exp(-kappa_ * t) + thermal);
        }
        return out;
    }

private:
    double kappa_;
    double temperature_;
};

} // namespace Sophia::Local