//! Agente Q-Learning com Q-Table LRU e decaimento de exploração.
//!
//! # Correções aplicadas
//! - **P8 FIX**: Q-Table com capacidade máxima (LRU) para evitar OOM em hardware espacial.
//! - **P10 FIX**: Taxa de exploração (ε) decai exponencialmente para garantir convergência.
//!
//! # Algoritmo
//! ```text
//! ε(t) = max(ε_min, ε_0 * exp(-t / τ))
//! Q(s,a) ← Q(s,a) + α * [r + γ * max_a' Q(s',a') - Q(s,a)]
//! ```

/// Estado do agente (discretizado).
///
/// Estado = (nó atual, nível de buffer, tempo restante do contato, tipo de nó destino)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct State {
    pub node_id: usize,
    pub buffer_level: u8,      // 0-9 (discretizado: buffer / cap * 10)
    pub contact_remaining: u8, // 0-9 (discretizado: tempo restante / 360)
    pub dst_type: u8,            // Tipo do nó destino codificado
}

impl State {
    /// Discretiza nível de buffer (0.0..1.0 → 0..9).
    pub fn discretize_buffer(buffer: usize, cap: usize) -> u8 {
        if cap == 0 {
            return 9;
        }
        ((buffer as f64 / cap as f64) * 9.0).min(9.0) as u8
    }

    /// Discretiza tempo restante de contato (0..3600s → 0..9).
    pub fn discretize_time_remaining(seconds: f64) -> u8 {
        ((seconds / 360.0).min(9.0)) as u8
    }
}

/// Ação do agente = próximo salto (nó vizinho).
pub type Action = usize;

/// Entrada da Q-Table com timestamp para LRU.
#[derive(Debug, Clone)]
struct QEntry {
    value: f64,
    last_access: u64, // Contador de passos
}

/// Agente Q-Learning com Q-Table LRU.
///
/// # Memória
/// - Capacidade máxima: `max_entries` (padrão: 10_000)
/// - Quando cheio, remove a entrada menos recentemente usada (LRU)
pub struct QLearningAgent {
    /// Taxa de aprendizado (α).
    pub alpha: f64,
    /// Fator de desconto (γ).
    pub gamma: f64,
    /// Taxa de exploração inicial (ε_0).
    pub epsilon_init: f64,
    /// Taxa de exploração mínima (ε_min).
    pub epsilon_min: f64,
    /// Constante de tempo de decaimento de ε (τ).
    pub epsilon_tau: f64,
    /// Capacidade máxima da Q-Table.
    pub max_entries: usize,
    /// Q-Table: (State, Action) → (valor, timestamp).
    q_table: hashbrown::HashMap<(State, Action), QEntry>,
    /// Contador de passos (para LRU e decaimento).
    step_counter: u64,
    /// Gerador de números aleatórios.
    rng: rand::rngs::SmallRng,
}

impl QLearningAgent {
    /// Cria agente com configuração padrão para deep space.
    pub fn new(seed: u64) -> Self {
        use rand::SeedableRng;
        Self {
            alpha: 0.1,
            gamma: 0.95,
            epsilon_init: 0.3,
            epsilon_min: 0.01,
            epsilon_tau: 10_000.0,
            max_entries: 10_000,
            q_table: hashbrown::HashMap::new(),
            step_counter: 0,
            rng: rand::rngs::SmallRng::seed_from_u64(seed),
        }
    }

    /// Taxa de exploração atual (decaimento exponencial).
    ///
    /// **P10 FIX**: ε decai exponencialmente, garantindo convergência.
    pub fn epsilon(&self) -> f64 {
        self.epsilon_min.max(
            self.epsilon_init * (-(self.step_counter as f64) / self.epsilon_tau).exp()
        )
    }

    /// Retorna o valor Q para um par (estado, ação).
    pub fn q_value(&self, state: State, action: Action) -> f64 {
        self.q_table
            .get(&(state, action))
            .map(|e| e.value)
            .unwrap_or(0.0)
    }

    /// Seleciona ação usando ε-greedy.
    pub fn select_action(&mut self, state: State, valid_actions: &[Action]) -> Action {
        if valid_actions.is_empty() {
            return 0; // Fallback
        }

        let eps = self.epsilon();

        if rand::Rng::gen::<f64>(&mut self.rng) < eps {
            // Exploração: ação aleatória
            let idx = rand::Rng::gen_range(&mut self.rng, 0..valid_actions.len());
            valid_actions[idx]
        } else {
            // Exploração: melhor ação conhecida
            let mut best_action = valid_actions[0];
            let mut best_value = self.q_value(state, best_action);

            for &action in &valid_actions[1..] {
                let value = self.q_value(state, action);
                if value > best_value {
                    best_value = value;
                    best_action = action;
                }
            }
            best_action
        }
    }

    /// Atualiza Q-Table com uma transição (s, a, r, s').
    ///
    /// # Fórmula
    /// ```text
    /// Q(s,a) ← Q(s,a) + α * [r + γ * max_a' Q(s',a') - Q(s,a)]
    /// ```
    pub fn update(
        &mut self,
        state: State,
        action: Action,
        reward: f64,
        next_state: State,
        next_actions: &[Action],
    ) {
        self.step_counter += 1;

        let current_q = self.q_value(state, action);

        // max_a' Q(s', a')
        let max_next_q = if next_actions.is_empty() {
            0.0
        } else {
            next_actions
                .iter()
                .map(|&a| self.q_value(next_state, a))
                .fold(f64::NEG_INFINITY, f64::max)
        };

        let new_q = current_q + self.alpha * (reward + self.gamma * max_next_q - current_q);

        // Inserir/atualizar entrada
        self.insert_q(state, action, new_q);
    }

    /// Insere valor Q com gerenciamento LRU.
    ///
    /// **P8 FIX**: Se capacidade excedida, remove entrada LRU.
    fn insert_q(&mut self, state: State, action: Action, value: f64) {
        // Se já existe, atualiza
        if let Some(entry) = self.q_table.get_mut(&(state, action)) {
            entry.value = value;
            entry.last_access = self.step_counter;
            return;
        }

        // Se cheio, remove LRU
        if self.q_table.len() >= self.max_entries {
            self.evict_lru();
        }

        self.q_table.insert(
            (state, action),
            QEntry {
                value,
                last_access: self.step_counter,
            },
        );
    }

    /// Remove a entrada menos recentemente usada.
    fn evict_lru(&mut self) {
        let lru_key = self
            .q_table
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(k, _)| *k);

        if let Some(key) = lru_key {
            self.q_table.remove(&key);
        }
    }

    /// Calcula recompensa para uma transição.
    ///
    /// # Componentes
    /// - `delivery`: +100 (bundle entregue)
    /// - `latency_penalty`: -latencia_normalizada * 10
    /// - `loss_penalty`: -perda * 50
    /// - `buffer_penalty`: -congestionamento * 20
    pub fn compute_reward(
        &self,
        delivered: bool,
        latency_sec: f64,
        packet_loss: f64,
        buffer_ratio: f64,
    ) -> f64 {
        if delivered {
            let latency_norm = (latency_sec / LIGHT_YEAR_SECONDS).min(10.0);
            100.0 - latency_norm * 10.0 - packet_loss * 50.0 - buffer_ratio * 20.0
        } else {
            -100.0 // Penalidade forte por perda
        }
    }

    /// Estatísticas do agente.
    pub fn stats(&self) -> AgentStats {
        AgentStats {
            q_table_size: self.q_table.len(),
            step_counter: self.step_counter,
            current_epsilon: self.epsilon(),
            max_entries: self.max_entries,
        }
    }
}

/// Estatísticas do agente Q-Learning.
#[derive(Debug, Clone)]
pub struct AgentStats {
    pub q_table_size: usize,
    pub step_counter: u64,
    pub current_epsilon: f64,
    pub max_entries: usize,
}

/// Constante: segundos em um ano-luz (para normalização).
const LIGHT_YEAR_SECONDS: f64 = 31_557_600.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epsilon_decay() {
        let mut agent = QLearningAgent::new(42);
        let eps0 = agent.epsilon();
        assert!((eps0 - 0.3).abs() < 0.01);

        // Simula 10.000 passos
        for _ in 0..10_000 {
            agent.step_counter += 1;
        }
        let eps1 = agent.epsilon();
        assert!(eps1 < eps0); // Decaiu
        assert!(eps1 >= agent.epsilon_min); // Não abaixo do mínimo
    }

    #[test]
    fn test_q_table_lru_eviction() {
        let mut agent = QLearningAgent::new(42);
        agent.max_entries = 5; // Capacidade pequena para teste

        let state = State {
            node_id: 0,
            buffer_level: 0,
            contact_remaining: 0,
            dst_type: 0,
        };

        // Insere 5 entradas
        for i in 0..5 {
            agent.insert_q(state, i, i as f64);
        }
        assert_eq!(agent.q_table.len(), 5);

        // Acessa a entrada 0 para atualizar timestamp
        agent.q_table.get_mut(&(state, 0)).unwrap().last_access = 100;

        // Insere a 6ª entrada — deve evict a LRU (não a 0, que foi acessada)
        agent.insert_q(state, 5, 5.0);
        assert_eq!(agent.q_table.len(), 5);
        assert!(agent.q_table.contains_key(&(state, 0))); // Ainda existe
        assert!(agent.q_table.contains_key(&(state, 5))); // Nova entrada
    }

    #[test]
    fn test_select_action_epsilon_greedy() {
        let mut agent = QLearningAgent::new(42);
        agent.epsilon_init = 0.0; // Sem exploração (determinístico)
        agent.epsilon_min = 0.0; // Desativa o piso (determinístico)

        let state = State {
            node_id: 0,
            buffer_level: 0,
            contact_remaining: 0,
            dst_type: 0,
        };

        // Define Q(s, 1) = 10.0, Q(s, 2) = 5.0
        agent.insert_q(state, 1, 10.0);
        agent.insert_q(state, 2, 5.0);

        let action = agent.select_action(state, &[1, 2]);
        assert_eq!(action, 1); // Deve escolher a de maior valor
    }

    #[test]
    fn test_reward_computation() {
        let agent = QLearningAgent::new(42);

        let r1 = agent.compute_reward(true, 0.0, 0.0, 0.0);
        assert!((r1 - 100.0).abs() < 0.1);

        let r2 = agent.compute_reward(true, LIGHT_YEAR_SECONDS, 0.1, 0.5);
        assert!(r2 < 100.0);
        assert!(r2 > -100.0);

        let r3 = agent.compute_reward(false, 0.0, 0.0, 0.0);
        assert_eq!(r3, -100.0);
    }

    #[test]
    fn test_discretization() {
        assert_eq!(State::discretize_buffer(0, 100), 0);
        assert_eq!(State::discretize_buffer(50, 100), 4); // ~4.5 truncado
        assert_eq!(State::discretize_buffer(100, 100), 9);
        assert_eq!(State::discretize_buffer(0, 0), 9); // Edge case
    }
}
