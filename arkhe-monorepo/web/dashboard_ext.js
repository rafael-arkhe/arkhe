// web/dashboard_ext.js
// Extensão do dashboard p5.js — Consolidação Tripla MEV + ANTH + ARKHE (bloco 1052).
//
// Módulo autônomo: define `updateUnifiedState()` e as funções de desenho por
// camada. Espera um objeto global `state` (como nos sketches p5.js da Catedral).
// Registe `updateUnifiedState()` no loop `draw()` (ou via setInterval).

/* global state */

// ─── MEV — MEV-001..006 ─────────────────────────────────────────────────────
function drawMevStatus() {
    fill(200);
    textAlign(LEFT, TOP);
    text('MEV Protection (MEV-001..006)', 20, 120);

    const privacy = state.mev && state.mev.privacy;
    fill(privacy ? '#4ade80' : '#f87171');
    text('MEV-001 Privacidade: ' + (privacy ? 'ATIVA' : 'INATIVA'), 20, 140);

    const atomicity = state.mev && state.mev.atomicity;
    fill(atomicity ? '#4ade80' : '#f87171');
    text('MEV-002 Atomicidade: ' + (atomicity ? 'ATIVA' : 'INATIVA'), 20, 160);

    const recovery = state.mev ? state.mev.recovery : 0;
    fill(recovery > 0 ? '#4ade80' : '#94a3b8');
    text('MEV-003 Recuperado: ' + recovery.toFixed(2), 20, 180);

    const searcher = state.mev && state.mev.searcher_validated;
    fill(searcher ? '#4ade80' : '#f87171');
    text('MEV-004 Searcher: ' + (searcher ? 'VALIDADO' : 'REJEITADO'), 20, 200);

    const chain = state.mev && state.mev.chain_compatible;
    fill(chain ? '#4ade80' : '#f87171');
    text('MEV-005 Multi-chain: ' + (chain ? 'COMPATIVEL' : 'INCOMPATIVEL'), 20, 220);

    const immutable = state.mev && state.mev.immutable;
    fill(immutable ? '#4ade80' : '#f87171');
    text('MEV-006 Imutabilidade: ' + (immutable ? 'GARANTIDA' : 'VIOLADA'), 20, 240);
}

// ─── ANTH — ANTH-001..006 ───────────────────────────────────────────────────
function drawAnthStatus() {
    fill(200);
    text('ANTH Protection (ANTH-001..006)', 20, 280);

    const formal = state.anth && state.anth.formal_verification;
    fill(formal ? '#4ade80' : '#f87171');
    text('ANTH-001 Verificacao Formal: ' + (formal ? 'ATIVA' : 'FALHOU'), 20, 300);

    const brake = state.anth && state.anth.momentum_brake;
    fill(brake ? '#4ade80' : '#f87171');
    text('ANTH-002 Travao de Momentum: ' + (brake ? 'ATIVO' : 'ATIVADO'), 20, 320);

    const sandbox = state.anth && state.anth.sandbox;
    fill(sandbox ? '#4ade80' : '#f87171');
    text('ANTH-003 Sandbox: ' + (sandbox ? 'SELADO' : 'VIOLADO'), 20, 340);

    const config = state.anth && state.anth.config_immutable;
    fill(config ? '#4ade80' : '#f87171');
    text('ANTH-004 Configuracao: ' + (config ? 'IMUTAVEL' : 'ALTERADA'), 20, 360);

    const runtime = state.anth && state.anth.runtime_sandbox;
    fill(runtime ? '#4ade80' : '#f87171');
    text('ANTH-005 Sandbox Runtime: ' + (runtime ? 'VERIFICADO' : 'VIOLADO'), 20, 380);

    const monitor = state.anth && state.anth.realtime_monitoring;
    fill(monitor ? '#4ade80' : '#f87171');
    text('ANTH-006 Monitoramento: ' + (monitor ? 'ATIVO' : 'INTERROMPIDO'), 20, 400);

    const alerts = state.anth && state.anth.alerts ? state.anth.alerts : [];
    if (alerts.length > 0) {
        fill('#f87171');
        text('ALERTAS: ' + alerts.length, 20, 420);
        for (let i = 0; i < Math.min(alerts.length, 3); i++) {
            text('  * ' + alerts[i], 30, 440 + i * 20);
        }
    }
}

// ─── ARKHE — I619, I622, I623, I624 ─────────────────────────────────────────
function drawArkheStatus() {
    fill(200);
    text('Arkhe Invariants (I619/I622/I623/I624)', 20, 520);

    const bpu = state.arkhe && state.arkhe.bpu;
    fill(bpu ? '#4ade80' : '#f87171');
    text('I619 (BPU): ' + (bpu ? 'DISPONIVEL' : 'INDISPONIVEL'), 20, 540);

    const sharding = state.arkhe && state.arkhe.sharding;
    fill(sharding ? '#4ade80' : '#f87171');
    text('I622 (Sharding): ' + (sharding ? 'DISPONIVEL' : 'INDISPONIVEL'), 20, 560);

    const energy = state.arkhe ? state.arkhe.energy : 0;
    fill(energy > 100000 ? '#4ade80' : '#f87171');
    text('I623 (Energia): ' + energy + ' (min 100000)', 20, 580);

    const key = state.arkhe && state.arkhe.key_valid;
    fill(key ? '#4ade80' : '#f87171');
    text('I624 (Chave): ' + (key ? 'VALIDA' : 'INVALIDA'), 20, 600);
}

// ─── Atualização periódica do estado ────────────────────────────────────────
function updateUnifiedState() {
    if (!state) state = {};

    state.mev = {
        privacy: true,
        atomicity: true,
        recovery: Math.random() * 10,
        searcher_validated: true,
        chain_compatible: true,
        immutable: true,
    };

    state.anth = {
        formal_verification: true,
        momentum_brake: true,
        sandbox: true,
        config_immutable: true,
        runtime_sandbox: Math.random() > 0.05,
        realtime_monitoring: Math.random() > 0.03,
        alerts: [],
    };
    if (!state.anth.runtime_sandbox) {
        state.anth.alerts.push('ANTH-005 runtime sandbox violation');
    }
    if (!state.anth.realtime_monitoring) {
        state.anth.alerts.push('ANTH-006 realtime monitoring interrupted');
    }

    state.arkhe = {
        bpu: true,
        sharding: true,
        energy: 500000 + Math.random() * 1000000,
        key_valid: true,
    };

    drawMevStatus();
    drawAnthStatus();
    drawArkheStatus();
}