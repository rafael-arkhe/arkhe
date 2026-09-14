// sophia_orchestrator.cpp — binário principal do orquestrador local.
// Compilar: see ../../CMakeLists.txt
#include <iostream>
#include <string>

#include "orchestration/local_agent_orchestrator.hpp"
#include "integration/temporal_bridge.hpp"

int main() {
    Sophia::Local::LocalAgentOrchestrator orchestrator;
    orchestrator.start();

    Sophia::Integration::TemporalBridge bridge("/var/lib/sophia/ledger_agents.log");

    for (int i = 0; i < 20; ++i) {
        orchestrator.runFullCycle();
        const auto m = orchestrator.metrics();
        std::cout << "ψ_C=" << m.psi_C
                  << " Q=" << m.q_coefficient
                  << " coherence=" << m.global_coherence
                  << " τ=" << m.decoherence_time
                  << " bounds=" << (m.in_bounds ? "OK" : "VIOLATION")
                  << std::endl;
        bridge.anchor("sophia_cycle", std::to_string(m.psi_C));
    }

    orchestrator.stop();
    return 0;
}