#pragma once
// Hardening do sistema (C++).
// Aplica desativação de serviços, firewall, permissões e auditoria.
#include <cstdlib>
#include <iostream>
#include <string>
#include <vector>

namespace Sophia::Security {

class SystemHardening {
public:
    std::vector<std::string> applyHardening() {
        std::vector<std::string> applied;

        // 1. Desabilitar serviços não essenciais
        run("sudo systemctl disable avahi-daemon bluetooth", applied);

        // 2. Firewall — política INPUT DROP + portas permitidas
        run("sudo iptables -P INPUT DROP", applied);
        run("sudo iptables -A INPUT -p tcp --dport 8080 -j ACCEPT", applied);
        run("sudo iptables -A INPUT -p tcp --dport 22 -j ACCEPT", applied);
        run("sudo iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT", applied);
        run("sudo iptables -A INPUT -i lo -j ACCEPT", applied);

        // 3. Permissões de arquivos sensíveis
        run("sudo chmod 600 /etc/sophia/secrets.conf", applied);

        // 4. Auditoria (auditd)
        run("sudo auditctl -w /opt/sophia/ -p wa -k sophia_changes", applied);

        return applied;
    }

private:
    static void run(const std::string& cmd, std::vector<std::string>& out) {
        out.push_back(cmd);
        std::cout << "[Hardening] " << cmd << std::endl;
        // NOTE: system() é usado apenas como scaffold; em produção substituir
        // por fork/exec com validação (RSI / arkhé-input sanitized).
        std::system(cmd.c_str());
    }
};

} // namespace Sophia::Security