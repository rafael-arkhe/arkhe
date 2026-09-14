"""cocotb testbench for afe_smith_cordic.sv.

Differential test: the DUT's per-channel coupling output is compared against the
bit-accurate golden model in model/cordic_ref.py. Covers the 4-quadrant TEST
item (negative I/Q) and the AXI4-Lite protocol TEST item (AR->R, AW/W/B).

Run (requires a simulator + cocotb, NOT available in the authoring env):
    # Icarus Verilog:
    pip install cocotb cocotb-bus
    make            # via the Makefile in this dir, SIM=icarus
    # or Verilator:
    make SIM=verilator
"""
import math
import os
import sys

import cocotb
from cocotb.clock import Clock
from cocotb.triggers import RisingEdge, Timer

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "model"))
from cordic_ref import smith_pipeline, Q_IN_FRAC, Q_OUT_FRAC  # noqa: E402

ONE_IN = 1 << Q_IN_FRAC
ONE_OUT = 1 << Q_OUT_FRAC
COUP_TOL = 4  # LSBs of Q16.16 slack over the model (RTL truncation parity)

ADDR_STATUS = 0x00
ADDR_CTRL = 0x04
ADDR_D_BASE = 0x08


async def _reset(dut):
    dut.rst_n.value = 0
    dut.adc_valid.value = 0
    dut.adc_i.value = 0
    dut.adc_q.value = 0
    for sig in ("s_axi_awvalid", "s_axi_wvalid", "s_axi_bready",
                "s_axi_arvalid", "s_axi_rready"):
        getattr(dut, sig).value = 0
    for _ in range(5):
        await RisingEdge(dut.clk)
    dut.rst_n.value = 1
    await RisingEdge(dut.clk)


async def _feed_channel(dut, i_val, q_val):
    """Drive one I/Q sample and wait for the CORDIC to finish that channel."""
    while not int(dut.adc_ready.value):
        await RisingEdge(dut.clk)
    dut.adc_i.value = i_val & ((1 << len(dut.adc_i)) - 1)
    dut.adc_q.value = q_val & ((1 << len(dut.adc_q)) - 1)
    dut.adc_valid.value = 1
    await RisingEdge(dut.clk)
    dut.adc_valid.value = 0
    # S_RUN (N_ITER) + S_DONE + a margin
    for _ in range(20):
        await RisingEdge(dut.clk)


async def _axi_read(dut, addr):
    dut.s_axi_araddr.value = addr
    dut.s_axi_arvalid.value = 1
    dut.s_axi_rready.value = 1
    await RisingEdge(dut.clk)
    while not int(dut.s_axi_arready.value):
        await RisingEdge(dut.clk)
    dut.s_axi_arvalid.value = 0
    while not int(dut.s_axi_rvalid.value):
        await RisingEdge(dut.clk)
    data = int(dut.s_axi_rdata.value)
    resp = int(dut.s_axi_rresp.value)
    await RisingEdge(dut.clk)
    dut.s_axi_rready.value = 0
    return data, resp


async def _axi_write(dut, addr, data):
    dut.s_axi_awaddr.value = addr
    dut.s_axi_awvalid.value = 1
    dut.s_axi_wdata.value = data
    dut.s_axi_wstrb.value = 0xF
    dut.s_axi_wvalid.value = 1
    dut.s_axi_bready.value = 1
    await RisingEdge(dut.clk)
    while not (int(dut.s_axi_awready.value) and int(dut.s_axi_wready.value)):
        await RisingEdge(dut.clk)
    dut.s_axi_awvalid.value = 0
    dut.s_axi_wvalid.value = 0
    while not int(dut.s_axi_bvalid.value):
        await RisingEdge(dut.clk)
    resp = int(dut.s_axi_bresp.value)
    await RisingEdge(dut.clk)
    dut.s_axi_bready.value = 0
    return resp


def _to_signed(v, bits):
    return v - (1 << bits) if v & (1 << (bits - 1)) else v


@cocotb.test()
async def test_four_quadrants(dut):
    """Feed 8 channels spanning all four quadrants; check coupling vs model."""
    cocotb.start_soon(Clock(dut.clk, 10, units="ns").start())  # 100 MHz
    await _reset(dut)

    # 8 test vectors: +,+ / -,+ / -,- / +,- / axes
    pts = [
        (int(0.6 * ONE_IN),  int(0.3 * ONE_IN)),
        (int(-0.5 * ONE_IN), int(0.4 * ONE_IN)),
        (int(-0.7 * ONE_IN), int(-0.2 * ONE_IN)),
        (int(0.2 * ONE_IN),  int(-0.6 * ONE_IN)),
        (ONE_IN - 1, 0),
        (0, ONE_IN - 1),
        (0, 0),
        (int(-0.35 * ONE_IN), int(-0.35 * ONE_IN)),
    ]
    for (i_val, q_val) in pts:
        await _feed_channel(dut, i_val, q_val)

    rw = len(dut.adc_i)
    for ch, (i_val, q_val) in enumerate(pts):
        data, resp = await _axi_read(dut, ADDR_D_BASE + 4 * ch)
        assert resp == 0, f"ch{ch} read RRESP={resp}"
        exp = smith_pipeline(_to_signed(i_val & ((1 << rw) - 1), rw),
                             _to_signed(q_val & ((1 << rw) - 1), rw))
        err = abs(data - exp)
        dut._log.info(f"ch{ch} ({i_val},{q_val}) rtl={data} model={exp} err={err}")
        assert err <= COUP_TOL, f"ch{ch}: coupling {data} vs model {exp} (err {err})"


@cocotb.test()
async def test_axi_protocol(dut):
    """STATUS reads, CTRL write with B response, and a decode-error read."""
    cocotb.start_soon(Clock(dut.clk, 10, units="ns").start())
    await _reset(dut)

    status, resp = await _axi_read(dut, ADDR_STATUS)
    assert resp == 0
    assert status & 0x1, "idle bit should be set after reset"

    bresp = await _axi_write(dut, ADDR_CTRL, 0x1)     # clear 'done'
    assert bresp == 0, f"CTRL write BRESP={bresp}"

    _, rresp = await _axi_read(dut, 0xFC)             # unmapped -> DECERR
    assert rresp == 0b11, f"expected DECERR on bad addr, got {rresp}"
