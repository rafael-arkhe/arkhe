// web/catedral_ext.js
// Extensão do Painel da Catedral — camadas constitucionais (bloco 1070+).
//
// Módulo autónomo: define `updateCatedralState()` e funções de desenho por
// camada. Espera um objeto global `state` (padrão dos sketches p5.js da
// Catedral). O sketch principal regista `updateCatedralState()` no loop
// `draw()`. Também hidrata o DOM lateral (ledger, substratos, selo).
//
// ⚠️ STATUS: PROTÓTIPO VISUAL (mockup). Todos os dados são ESTÁTICOS —
// simulados no cliente. NÃO conectado a nenhum backend /api/v1. Nenhuma
// cadeia verificada criptograficamente aqui.
//
// Banda Φ_C (Gap-1) — origem documentada em docs/coherence_metric.md §1:
//   - piso 0.577350 = truncamento de 1/√3 = 0.5773502691… (constante mágica,
//     blob par packages/arkhe-field-stability/src/phi.rs:98 GAP1_INFERIOR);
//   - teto 0.999900 = 1 − 10⁻⁴ (Φ=1 ideal assintótico, jamais certificável:
//     Λ=1 exigiria latência nula — docs/coherence_metric.md:17).

/* global state */

// ─── Dados estáticos: 19 Invariantes Constitucionais ────────────────────────
const CATEDRAL_INVARIANTS = [
    ['Ghost-1',      'Integridade de substrato',    'manifest.sha3 bate com hashes'],
    ['Ghost-2',      'Selo de manifesto',           'container sealed'],
    ['Ghost-3',      'Verificação cross-substrate', 'dependências resolvem'],
    ['Loopseal-1',   'Âncora TemporalChain',        'toda ação ancorada'],
    ['Loopseal-2',   'Imutabilidade do proof log',  'append-only, tamper-evident'],
    ['Loopseal-3',   'Audit trail completo',        'rastro integral'],
    ['Gap-1',        'Bound Φ_C',                   '0.577350 < Φ ≤ 0.999900'],
    ['Gap-2',        'Entropy budget',              'entropia criptográfica'],
    ['Gap-3',        'Consistência dimensional',    'invariantes × pesos'],
    ['Runtime-1',    'Isolamento de container',     'runtime isolado'],
    ['Runtime-2',    'Integridade venv',            '/arkhe/venv isolado'],
    ['Runtime-3',    'Healthcheck',                 'passa a cada 60s'],
    ['Ethics-1',     'Alinhamento 227-F',           'constitucional'],
    ['Ethics-2',     'Data minimization',           'só o necessário'],
    ['Simplicity-1', 'Complexidade ciclomática',    'abaixo do limiar'],
    ['Simplicity-2', 'Superfície de dependências',  'árvore mínima auditada'],
    ['Correlation-1','Referências cruzadas',        'verificadas'],
    ['Gravity-1',    'Consistência temporal',       'timestamps monotónicos'],
    ['Provenance-1', 'Notarização TLSNotary',       'comms notarizadas'],
];

// ─── Substratos ativos (mais recentes) ──────────────────────────────────────
const CATEDRAL_SUBSTRATOS = [
    { id: '546-LASER-PHOTONIC-ENGINE',        v: 'v1.1', phi: 0.994 },
    { id: '565-TLSNOTARY-BRIDGE',             v: 'v2.0', phi: 0.999 },
    { id: '569-TELEPORT-QUANTUM-LINK',        v: 'v1.0', phi: 0.988 },
    { id: '570-CLAUDE-CODE-ORCHESTRATOR',     v: 'v1.0', phi: 0.984 },
    { id: 'KSM680-TORUS-TELEPORT',            v: 'v1.0', phi: 0.976 },
    { id: '924-POTT-INTERPLANETARY-TRANSPORT',v: 'v359.0', phi: 0.990 },
    { id: '972-ARKHE-BITCOIN',                v: 'v360.0', phi: 0.997 },
    { id: 'FIELD-STABILITY-COHERENCE',        v: 'v375.5', phi: 0.984 },
];

const CATEDRAL_SELO =
    'f595dfe1eb4d651749b9faa04aaf585f4f7f8de590b5899e20b252989eeec861';

// ─── Fonte de dados ─────────────────────────────────────────────────────────
// DADOS: ESTÁTICOS (mock). Fontes reais disponíveis (não conectadas):
//   Φ_C real ..... CoherenceLedger (crate arkhe-field-stability, Fase 4)
//   Cadeia real .. bloco_*.json encadeados por hash_anterior (git SHA-256)
const CATEDRAL_DATA_ORIGIN = 'MOCK · dados estáticos · sem /api/v1';

// ─── Camada de rótulo de origem (honestidade) ───────────────────────────────
function drawDataOrigin() {
    fill('#fbbf24');
    textAlign(LEFT, TOP);
    textSize(9);
    text('FONTE: ' + CATEDRAL_DATA_ORIGIN, 24, height - 18);
}

// ─── Camada Φ_C (Gap-1) ─────────────────────────────────────────────────────
function drawPhiGauge(phi) {
    const x = 120, y = 150, r = 64;

    // Arco base (banda constitucional Gap-1)
    push();
    strokeWeight(12);
    stroke('#1e2740');
    noFill();
    arc(x, y, r * 2, r * 2, PI, TWO_PI);

    // Setor válido: 0.577350 (180°) .. 0.999900 (~359°)
    const start = map(0.577350, 0.577350, 0.999900, PI, TWO_PI);
    const stop  = map(constrain(phi, 0.577350, 0.999900), 0.577350, 0.999900, PI, TWO_PI);
    stroke(phi >= 0.98 ? '#4ade80' : (phi > 0.577350 ? '#fbbf24' : '#f87171'));
    arc(x, y, r * 2, r * 2, start, stop);
    pop();

    fill(phi >= 0.98 ? '#4ade80' : phi > 0.577350 ? '#fbbf24' : '#f87171');
    textAlign(CENTER, CENTER);
    textStyle(BOLD);
    textSize(26);
    text(phi.toFixed(4), x, y - 8);
    textStyle(NORMAL);
    textSize(10);
    fill(150);
    text('Φ_C · Gap-1', x, y + 18);
    text('jurisdição [0.577350, 0.999900]', x, y + 32);
}

// ─── Camada dos 19 invariantes ──────────────────────────────────────────────
function drawInvariantsGrid() {
    const cols = 4, cellW = 118, cellH = 30;
    const x0 = 24, y0 = 40;
    textAlign(LEFT, TOP);
    fill('#c7d2fe');
    textSize(11);
    text('INVARIANTES CONSTITUCIONAIS (19/19)', x0, 14);

    noStroke();
    CATEDRAL_INVARIANTS.forEach(function (inv, i) {
        const col = i % cols, row = Math.floor(i / cols);
        const x = x0 + col * cellW, y = y0 + row * cellH;
        const ok = state.invariantes[inv[0]] !== false;
        fill(ok ? '#0d1120' : '#2a1420');
        rect(x, y, cellW - 6, cellH - 6, 3);
        fill(ok ? '#4ade80' : '#f87171');
        ellipse(x + 8, y + 10, 5.5, 5.5);
        fill('#e2e8f0');
        textSize(9);
        text(inv[0], x + 16, y + 5);
        fill(ok ? '#94a3b8' : '#fbbf24');
        textSize(7);
        text(ok ? 'PASS' : 'VIOLADO', x + 16, y + 16);
    });
}

// ─── Camada do ledger (TemporalChain) ───────────────────────────────────────
function drawLedgerMini() {
    fill('#c7d2fe');
    textSize(11);
    textAlign(LEFT, TOP);
    text('TEMPORALCHAIN · LEDGER (append-only)', 24, 196);

    push();
    textAlign(LEFT, TOP);
    noStroke();
    fill('#1e2740');
    const barW = 280, barH = 56;
    rect(24, 212, barW, barH, 4);

    const entries = state.chain;
    for (let i = 0; i < Math.min(entries.length, 5); i++) {
        fill(entries[i].ok ? '#4ade80' : '#f87171');
        ellipse(24 + 12 + i * 16, 224, 7, 7);
    }
    fill('#94a3b8');
    textSize(8);
    text(entries.length + ' blocos selados · hashes SHA-256 encadeados', 24 + 90, 222);
    const last = entries[entries.length - 1];
    if (last) {
        fill('#c7d2fe');
        textSize(9);
        text('última âncora: bloco ' + last.bloco + ' (' + last.nome + ')', 24 + 9, 248);
    }
    pop();
}

// ─── Camada de healthcheck (Runtime-3) ──────────────────────────────────────
function drawHealth() {
    push();
    translate(24, 300);
    const healthy = state.health.runtime && state.health.audit === null;
    fill(healthy ? '#4ade80' : '#f87171');
    ellipse(0, 0, 12, 12);
    fill('#c7d2fe');
    textSize(10);
    textAlign(LEFT, CENTER);
    text('Healthcheck: ' + (healthy ? 'OK' : 'FALHA') + ' · proc. (Runtime-3)', 20, 0);

    fill(state.health.runtime ? '#4ade80' : '#f87171');
    text('venv /arkhe/venv (Runtime-2): ' + (state.health.runtime ? 'ISOLADO' : 'VIOLADO'), 20, 20);

    fill(state.health.audit === null ? '#4ade80' : '#f87171');
    text('loopseal-2 proof log: ' + (state.health.audit === null ? 'IMUTÁVEL' : 'ALTERADO'), 20, 40);

    const pg = state.health.pg;
    fill(pg ? '#4ade80' : '#f87171');
    text('provenance-1 tlsnotary: ' + (pg ? 'NOTARIZADO' : 'PENDENTE'), 20, 60);
    pop();
}

// ─── Atualização periódica do estado + hidratação DOM ───────────────────────
function updateCatedralState() {
    if (!state) state = {};
    if (!state._catedral) state._catedral = { phi: 0.970, chain: null };

    // Φ_C com leve ruído na banda constitucional (jitter < 10%, faixa operacional)
    const base = state._catedral.phi;
    const jitter = (Math.random() - 0.47) * 0.003;
    state._catedral.phi = constrain(base + jitter, 0.977, 0.999);

    state.invariantes = state.invariantes || {};
    state.chain = state.chain || [];
    state.health = state.health || { runtime: true, audit: null, pg: true };

    // Ledger append (padrão Loopseal-2: só cresce, nunca reescreve)
    if (!state._catedral.chain) {
        state._catedral.chain = true;
        const ledger = [
            { bloco: 1058, nome: 'errata-chain closeout',      ok: true },
            { bloco: 1052, nome: 'consolidação MEV+ANTH+ARKHE', ok: true },
            { bloco: 1008, nome: 'ponte consciência host',      ok: true },
            { bloco: 1009, nome: 'lean-bridge',                 ok: true },
            { bloco: 1010, nome: 'orchestrator-gate',           ok: true },
            { bloco: 1011, nome: 'camada gpu',                  ok: true },
            { bloco: 1070, nome: 'registro ratificado',         ok: true },
        ];
        state.chain = ledger.concat(state.chain);
    }

    drawPhiGauge(state._catedral.phi);
    drawInvariantsGrid();
    drawLedgerMini();
    drawHealth();
    drawDataOrigin();
    hydrateDom();
}

// ─── Hidratação do DOM lateral (fora do canvas p5) ──────────────────────────
function hydrateDom() {
    const setText = function (id, val) {
        const el = document.getElementById(id);
        if (el) el.textContent = val;
    };

    setText('stat-phi', state._catedral.phi.toFixed(4));
    setText('stat-bloco', String(state.chain[state.chain.length - 1].bloco));
    setText('stat-sub', String(CATEDRAL_SUBSTRATOS.length));
    setText('stat-inv', '19/19');

    // Ledger
    const chainEl = document.getElementById('chain');
    if (chainEl && chainEl.childElementCount === 0) {
        for (let i = state.chain.length - 1; i >= 0; i--) {
            const li = document.createElement('li');
            const dot = document.createElement('span');
            dot.className = 'pill ' + (state.chain[i].ok ? 'ok' : 'bad');
            li.appendChild(dot);
            const t = document.createElement('span');
            t.className = 'ts';
            t.textContent = '#' + state.chain[i].bloco + ' ' + state.chain[i].nome;
            li.appendChild(t);
            chainEl.appendChild(li);
        }
    }

    // Substratos
    const subEl = document.getElementById('subs');
    if (subEl && subEl.childElementCount === 0) {
        CATEDRAL_SUBSTRATOS.forEach(function (s) {
            const li = document.createElement('li');
            const dot = document.createElement('span');
            dot.className = 'pill ' + (s.phi >= 0.577350 ? 'ok' : 'bad');
            li.appendChild(dot);
            li.appendChild(document.createTextNode(s.id + ' ' + s.v + ' Φ ' + s.phi.toFixed(3)));
            subEl.appendChild(li);
        });
    }

    // Selo
    const se = document.getElementById('selo');
    if (se && se.childElementCount === 0) {
        se.textContent = CATEDRAL_SELO;
    }

    const clk = document.getElementById('clock');
    if (clk) clk.textContent = new Date().toLocaleTimeString('pt-BR');
}