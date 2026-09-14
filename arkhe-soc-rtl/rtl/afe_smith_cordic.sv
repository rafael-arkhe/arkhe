// SPDX-License-Identifier: MIT
// afe_smith_cordic.sv
// Smith-chart AFE mapper: (I,Q) -> coupling = 1 - |Gamma|^2, via a
// time-multiplexed magnitude-only CORDIC (vectoring mode, no angle accumulation).
//
// Golden model  : model/cordic_ref.py  (bit-accurate; RTL must match it)
// Fixed-point contract (see README.md, section "Q-format map" / item M5):
//   i_in, q_in         signed Q2.13   (1.0 == 8192)  -- Re/Im of reflection coeff.
//   internal x,y       signed, INT_W  -- Q2.13 scale, widened for CORDIC gain K
//   magnitude          signed Q2.13   -- descaled by 1/K (INV_K, Q0.15)
//   domain_d[ch]       unsigned Q16.16 (1.0 == 65536) -- 1 - |Gamma|^2, clipped
//
// Fixes vs the v26.3 draft:
//   C1  first-quadrant fold |i|,|q| before CORDIC -> all four quadrants converge.
//   C3  rotation direction is a combinational net (dir_pos = ~y[MSB]), not a
//       1-bit `reg signed` that could only ever hold {0,-1}.
//   (dropped) atan LUT + z accumulation: magnitude-only core does not need them,
//       which also removes the draft's INV_K/atan scaling ambiguity.
//   C2  full AXI4-Lite write response channel (bvalid/bresp) implemented.
//   M6  AR->R read path pipelined to a single cycle, VALID independent of READY.

module afe_smith_cordic #(
    parameter int unsigned IN_W        = 16,     // signed input width (Q2.13 fits +/-4.0)
    parameter int unsigned INT_W       = 24,     // internal CORDIC datapath width
    parameter int unsigned N_ITER      = 12,     // CORDIC iterations
    parameter int unsigned CH          = 8,      // channels = domain D nodes
    parameter int unsigned INV_K       = 19898,  // round((1/K) * 2^15), Q0.15
    parameter int unsigned INV_K_SHIFT = 15
)(
    input  logic                clk,
    input  logic                rst_n,

    // AFE I/Q stream (one channel per accepted sample, in ascending channel order)
    input  logic signed [IN_W-1:0] adc_i,
    input  logic signed [IN_W-1:0] adc_q,
    input  logic                   adc_valid,
    output logic                   adc_ready,   // high when core can accept a sample

    // AXI4-Lite slave (read: status + domain_d[0..7]; write: control)
    input  logic [7:0]  s_axi_awaddr,
    input  logic        s_axi_awvalid,
    output logic        s_axi_awready,
    input  logic [31:0] s_axi_wdata,
    input  logic [3:0]  s_axi_wstrb,
    input  logic        s_axi_wvalid,
    output logic        s_axi_wready,
    output logic [1:0]  s_axi_bresp,
    output logic        s_axi_bvalid,
    input  logic        s_axi_bready,
    input  logic [7:0]  s_axi_araddr,
    input  logic        s_axi_arvalid,
    output logic        s_axi_arready,
    output logic [31:0] s_axi_rdata,
    output logic [1:0]  s_axi_rresp,
    output logic        s_axi_rvalid,
    input  logic        s_axi_rready
);
    // ---------------- register map ----------------
    localparam logic [7:0] ADDR_STATUS  = 8'h00;  // RO : bit0 idle, bit2 done
    localparam logic [7:0] ADDR_CTRL    = 8'h04;  // RW : bit0 clears 'done'
    localparam logic [7:0] ADDR_D_BASE  = 8'h08;  // RO : domain_d[0..7], +4 each

    localparam logic [1:0] RESP_OKAY    = 2'b00;
    localparam logic [1:0] RESP_DECERR  = 2'b11;

    // Q16.16 constant 1.0
    localparam logic signed [31:0] ONE_Q16 = 32'sh0001_0000;

    // ================= CORDIC datapath =================
    typedef enum logic [1:0] {S_IDLE, S_RUN, S_DONE} state_t;
    state_t                     state;
    logic signed [INT_W-1:0]    x, y;
    logic [$clog2(N_ITER+1)-1:0] iter;
    logic [$clog2(CH)-1:0]      ch_sel;
    logic                       all_done;   // full D vector captured at least once

    logic [31:0] domain_d [0:CH-1];

    // combinational rotation direction (C3): drive y toward zero
    wire dir_pos = ~y[INT_W-1];             // y >= 0
    wire signed [INT_W-1:0] x_sh = x >>> iter;
    wire signed [INT_W-1:0] y_sh = y >>> iter;

    // |i|,|q| sign fold (C1), sign-extended to INT_W
    wire signed [INT_W-1:0] abs_i = adc_i[IN_W-1] ? -{{(INT_W-IN_W){1'b1}}, adc_i}
                                                  :  {{(INT_W-IN_W){1'b0}}, adc_i};
    wire signed [INT_W-1:0] abs_q = adc_q[IN_W-1] ? -{{(INT_W-IN_W){1'b1}}, adc_q}
                                                  :  {{(INT_W-IN_W){1'b0}}, adc_q};

    assign adc_ready = (state == S_IDLE);

    // descale + square + coupling (combinational, evaluated in S_DONE entry)
    wire signed [INT_W+16:0] mag_scaled = $signed(x) * $signed(INV_K);      // Q(2.13)*Q0.15
    wire signed [31:0]       mag_q13    = mag_scaled >>> INV_K_SHIFT;         // back to Q2.13
    wire signed [31:0]       mag_q16    = mag_q13 <<< 3;                      // Q2.13 -> Q16.16
    wire [63:0]              gamma_sq_w = ($unsigned(mag_q16) * $unsigned(mag_q16)) >> 16;
    wire signed [32:0]       coup_raw   = ONE_Q16 - $signed({1'b0, gamma_sq_w[31:0]});
    wire [31:0]              coup_clip  = (coup_raw < 0)                ? 32'd0 :
                                          (coup_raw > ONE_Q16)          ? 32'h0001_0000 :
                                                                          coup_raw[31:0];

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state    <= S_IDLE;
            x        <= '0;
            y        <= '0;
            iter     <= '0;
            ch_sel   <= '0;
            all_done <= 1'b0;
            for (int k = 0; k < CH; k++) domain_d[k] <= '0;
        end else begin
            case (state)
                S_IDLE: begin
                    if (adc_valid) begin
                        x    <= abs_i;      // first-quadrant seed (C1)
                        y    <= abs_q;
                        iter <= '0;
                        state <= S_RUN;
                    end
                end
                S_RUN: begin
                    if (dir_pos) begin
                        x <= x + y_sh;      // uses OLD x,y (non-blocking)
                        y <= y - x_sh;
                    end else begin
                        x <= x - y_sh;
                        y <= y + x_sh;
                    end
                    if (iter == N_ITER-1)
                        state <= S_DONE;
                    else
                        iter <= iter + 1'b1;
                end
                S_DONE: begin
                    domain_d[ch_sel] <= coup_clip;
                    ch_sel   <= (ch_sel == CH-1) ? '0 : ch_sel + 1'b1;  // wrap (C-fix)
                    all_done <= (ch_sel == CH-1);                       // pulse on full vector
                    state    <= S_IDLE;
                end
                default: state <= S_IDLE;
            endcase
        end
    end

    // ================= AXI4-Lite write channel (C2) =================
    logic aw_hs, w_hs;
    assign s_axi_awready = ~s_axi_bvalid;   // accept address when no pending response
    assign s_axi_wready  = ~s_axi_bvalid;
    assign aw_hs = s_axi_awvalid & s_axi_awready;
    assign w_hs  = s_axi_wvalid  & s_axi_wready;

    logic done_clear;
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            s_axi_bvalid <= 1'b0;
            s_axi_bresp  <= RESP_OKAY;
            done_clear   <= 1'b0;
        end else begin
            done_clear <= 1'b0;
            if (aw_hs && w_hs) begin
                s_axi_bvalid <= 1'b1;
                s_axi_bresp  <= (s_axi_awaddr == ADDR_CTRL) ? RESP_OKAY : RESP_DECERR;
                if (s_axi_awaddr == ADDR_CTRL && s_axi_wstrb[0] && s_axi_wdata[0])
                    done_clear <= 1'b1;               // CTRL.bit0 clears 'done'
            end else if (s_axi_bvalid && s_axi_bready) begin
                s_axi_bvalid <= 1'b0;                  // response consumed
            end
        end
    end

    // 'done' status latch, set by all_done, cleared by CTRL write
    logic done_flag;
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)          done_flag <= 1'b0;
        else if (done_clear) done_flag <= 1'b0;
        else if (all_done)   done_flag <= 1'b1;   // all_done pulses at end of each D vector
    end

    // ================= AXI4-Lite read channel (M6: 1-cycle AR->R) =================
    logic [31:0] rdata_mux;
    always_comb begin
        unique case (s_axi_araddr)
            ADDR_STATUS: rdata_mux = {29'd0, done_flag, 1'b0, (state == S_IDLE)};
            ADDR_CTRL:   rdata_mux = 32'd0;
            default: begin
                // domain_d[0..7] at ADDR_D_BASE + 4*ch
                if (s_axi_araddr >= ADDR_D_BASE &&
                    s_axi_araddr <  ADDR_D_BASE + 8'(4*CH) &&
                    s_axi_araddr[1:0] == 2'b00)
                    rdata_mux = domain_d[(s_axi_araddr - ADDR_D_BASE) >> 2];
                else
                    rdata_mux = 32'hDEAD_0000;
            end
        endcase
    end

    wire ar_addr_ok = (s_axi_araddr == ADDR_STATUS) || (s_axi_araddr == ADDR_CTRL) ||
                      ((s_axi_araddr >= ADDR_D_BASE) &&
                       (s_axi_araddr < ADDR_D_BASE + 8'(4*CH)) &&
                       (s_axi_araddr[1:0] == 2'b00));

    assign s_axi_arready = ~s_axi_rvalid;   // ready when no read result is waiting
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            s_axi_rvalid <= 1'b0;
            s_axi_rdata  <= 32'd0;
            s_axi_rresp  <= RESP_OKAY;
        end else begin
            if (s_axi_arvalid && s_axi_arready) begin
                s_axi_rvalid <= 1'b1;                       // VALID set independent of RREADY (M6)
                s_axi_rdata  <= rdata_mux;
                s_axi_rresp  <= ar_addr_ok ? RESP_OKAY : RESP_DECERR;
            end else if (s_axi_rvalid && s_axi_rready) begin
                s_axi_rvalid <= 1'b0;                       // read data consumed
            end
        end
    end

endmodule
