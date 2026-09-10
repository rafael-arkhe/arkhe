// web/catedral_sketch.js
// Sketch raiz do Painel da Catedral — registra a camada de extensão
// `updateCatedralState()` no loop `draw()` (padrão p5 da Catedral).

/* global state, updateCatedralState */

let state = {};

function setup() {
    const wrap = document.getElementById('canvas-wrap');
    const canvas = createCanvas(760, 420);
    canvas.parent(wrap);
    frameRate(10);
    textFont('ui-monospace, Menlo, Consolas, monospace');
}

function draw() {
    background('#07090f');
    drawFrame();
    if (typeof updateCatedralState === 'function') {
        updateCatedralState();
    }
}

function drawFrame() {
    stroke('#1e2740');
    strokeWeight(1);
    noFill();
    rect(1, 1, width - 2, height - 2);
    noStroke();
    fill('#64748b');
    textAlign(RIGHT, BOTTOM);
    textSize(8);
    text('arkhe · constitutional container runtime · v552.1', width - 10, height - 6);
}