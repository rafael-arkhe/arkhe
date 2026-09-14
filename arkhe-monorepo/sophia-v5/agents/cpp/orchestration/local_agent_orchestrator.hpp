#pragma once
// Orquestrador de agentes locais (C++).
// Executa os patches #19–22 em loop, coleta métricas e oferece API local.
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdlib>
#include <iostream>
#include <thread>
#include <utility>
#include <vector>

#include "patches/patch_19_unified_field.hpp"
#include "patches/patch_20_q_coefficient.hpp"
#include "patches/patch_21_global_brainwave.hpp"
#include "patches/patch_22_microtubule_qed.hpp"

namespace Sophia::Local {

struct AgentMetrics {
    double psi_C;
    double q_coefficient;
    double global_coherence;
    double decoherence_time;
    bool in_bounds;
};

class LocalAgentOrchestrator {
public:
    LocalAgentOrchestrator() : running_(false) {}

    void start() {
        running_ = true;
        agent_thread_ = std::thread(&LocalAgentOrchestrator::runLoop, this);
        std::cout << "[Sophia] Orquestrador local iniciado." << std::endl;
    }

    void stop() {
        running_ = false;
        if (agent_thread_.joinable()) agent_thread_.join();
        std::cout << "[Sophia] Orquestrador local finalizado." << std::endl;
    }

    double getPsiC() const { return unified_field_.psi_C(); }
    double getQCoefficient() const { return q_coefficient_.qValue(); }
    double getGlobalCoherence() const { return brainwave_.globalCoherence(); }
    double getMicrotubuleDecoherence() const { return microtubule_qed_.decoherenceTime(); }

    // Executa um ciclo completo de todos os agentes (parcialmente estocástico).
    void runFullCycle() {
        const double r1 = rand01();
        const double r2 = rand01();

        // 1. Patch #19 — atualizar campo ψ_C
        unified_field_.step(r1);

        // 2. Patch #20 — estudo twin simulado localmente
        std::vector<double> responses{0.50, 0.60, 0.44, 0.51, 0.57};
        q_coefficient_.measureFromTwinStudy(responses);

        // 3. Patch #21 — coerência global
        brainwave_.step(r2);

        // 4. Patch #22 — cavidade QED com κ dependente da coerência
        const double kappa = 1e5 * (1.0 - 0.5 * brainwave_.globalCoherence());
        microtubule_qed_.setKappa(kappa);
        (void)microtubule_qed_.simulateCavityDynamics(1e-7, 5);
    }

    AgentMetrics metrics() const {
        return {getPsiC(), getQCoefficient(), getGlobalCoherence(),
                getMicrotubuleDecoherence(), unified_field_.inBounds()};
    }

private:
    static double rand01() {
        return static_cast<double>(std::rand()) / static_cast<double>(RAND_MAX);
    }

    void runLoop() {
        using namespace std::chrono_literals;
        while (running_) {
            runFullCycle();
            std::this_thread::sleep_for(100ms); // 10 Hz
        }
    }

    std::atomic<bool> running_;
    std::thread agent_thread_;

    UnifiedCoherenceField unified_field_;
    QuantumEntanglementCoefficient q_coefficient_;
    GlobalBrainwaveNetwork brainwave_;
    MicrotubuleQEDCavity microtubule_qed_;
};

} // namespace Sophia::Local