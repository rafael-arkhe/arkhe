// ============================================================
// ctc_detector_enhanced_v2_6.v
// Detector de CTC (BLOCK 11 v2.6) - traducao Verilog do VHDL
// Arithmetica Q8.24 / LUT sigmoide de 256 entradas.
// Gerado por gen_ctc_verilog.py a partir de
// ctc_detector_enhanced_v2_6.vhd + qdiv_pipe.vhd + sigmoid_lut_pkg.vhd.
// Divisao pipelineada (12 estagios x 2 iteracoes) para timing a 100 MHz.
// ============================================================
`default_nettype none

// ------------------------------------------------------------
// qdiv_pipe: divisor Q8.24 pipelineado (resto-restaurador)
// Latencia: STAGES ciclos entre valid_in e valid_out.
// ------------------------------------------------------------
module qdiv_pipe #(parameter STAGES = 12, parameter ITERS = 2) (
    input  wire             clk,
    input  wire             rst_n,
    input  wire             valid_in,
    input  wire signed [31:0] num,
    input  wire signed [31:0] den,
    output wire             valid_out,
    output wire signed [31:0] quot
);

  localparam signed [31:0] SATURATE_Q = 32'sh01000000;  // 1.0 em Q8.24

  reg [32:0] r_pipe [0:STAGES];
  reg [31:0] q_pipe [0:STAGES];
  reg [32:0] b_pipe [0:STAGES];
  reg        n_pipe [0:STAGES];
  reg        s_pipe [0:STAGES];
  reg        v_pipe [0:STAGES];
  reg        valid_o;  // valid registrado (saida estavel, sem corrida)

  integer i;
  integer j;
  reg [32:0] rv;
  reg [31:0] qv;
  reg [31:0] A, B;
  reg        v_last;  // valid do ultimo estagio capturado pre-NBA

  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      for (i = 0; i <= STAGES; i = i + 1) begin
        r_pipe[i] <= 33'd0;
        q_pipe[i] <= 32'd0;
        b_pipe[i] <= 33'd0;
        n_pipe[i] <= 1'b0;
        s_pipe[i] <= 1'b0;
        v_pipe[i] <= 1'b0;
      end
      valid_o <= 1'b0;
    end else begin
      v_last = v_pipe[STAGES];  // leitura pre-NBA (deterministica)
      for (i = 0; i < STAGES; i = i + 1) begin
        v_pipe[i + 1] <= v_pipe[i];
        n_pipe[i + 1] <= n_pipe[i];
        s_pipe[i + 1] <= s_pipe[i];
      end
      for (i = 0; i < STAGES; i = i + 1) begin
        rv = r_pipe[i];
        qv = q_pipe[i];
        for (j = 0; j < ITERS; j = j + 1) begin
          rv = rv << 1;
          if (rv >= b_pipe[i]) begin
            rv = rv - b_pipe[i];
            qv[23 - i * ITERS - j] = 1'b1;
          end
        end
        r_pipe[i + 1] <= rv;
        q_pipe[i + 1] <= qv;
        b_pipe[i + 1] <= b_pipe[i];
      end
      if (valid_in) begin
        A = (num[31]) ? (~num + 32'd1) : num[31:0];
        B = (den[31]) ? (~den + 32'd1) : den[31:0];
        b_pipe[0] <= {1'b0, B};
        r_pipe[0] <= {1'b0, A};
        q_pipe[0] <= 32'd0;
        n_pipe[0] <= num[31] ^ den[31];
        s_pipe[0] <= (den == 32'sd0) || (A >= B);
        v_pipe[0] <= 1'b1;
      end else begin
        v_pipe[0] <= 1'b0;
      end
      // Valid registrado no mesmo bloco que o pipeline: usa v_last,
      // valor de v_pipe[STAGES] lido antes de qualquer NBA (sem corrida).
      valid_o <= v_last;
    end
  end

  wire signed [31:0] quot_i = s_pipe[STAGES] ? SATURATE_Q : $signed(q_pipe[STAGES]);
  assign quot      = n_pipe[STAGES] ? (-quot_i) : quot_i;

  assign valid_out = valid_o;

endmodule

// ------------------------------------------------------------
// ctc_detector_enhanced_v2_6: topo com divisores pipelineados
// ------------------------------------------------------------
module ctc_detector_enhanced_v2_6 (
    input  wire             clk,
    input  wire             rst_n,
    input  wire             valid_in,
    input  wire signed [31:0] purity_p,
    input  wire signed [31:0] ricci_tilde,
    input  wire signed [31:0] kolmogorov_cd,
    input  wire signed [31:0] meso,
    input  wire [7:0]       mode_n,
    output reg              valid_out,
    output reg  signed [31:0] retro_capacity,
    output reg  signed [31:0] retro_eff,
    output reg  signed [31:0] pctc_prob,
    output reg  signed [31:0] eff_eff,
    output reg  signed [31:0] torsion,
    output reg  signed [31:0] horizon,
    output wire             ctc_flag,
    output wire             retro_active,
    output wire             horizon_alarm
);

  localparam signed [31:0] ONE_Q         = 32'sh01000000;  // 1.0
  localparam signed [31:0] X_MIN_Q       = -32'sh08000000; // -8.0
  localparam signed [31:0] X_MAX_Q       = 32'sh08000000;  // +8.0
  localparam signed [31:0] ALPHA_Q = 32'sh00666666;
  localparam signed [31:0] BETA_Q = 32'sh004ccccd;
  localparam signed [31:0] GAMMA_Q = 32'sh00333333;
  localparam signed [31:0] DELTA_Q = 32'sh0019999a;
  localparam signed [31:0] KAPPA_Q = 32'sh00028f5c;
  localparam signed [31:0] P_OPERATE_Q = 32'sh0007d56a;
  localparam signed [31:0] INV_SIGMA_P_Q = 32'sh64000000;
  localparam signed [31:0] INV_TAU_CRIT_Q = 32'sh06aaaaab;
  localparam signed [31:0] HALF_Q = 32'sh00800000;
  localparam signed [31:0] HORIZON_ABORT_Q = 32'sh00b33333;
  localparam signed [31:0] RETRO_ON_Q = 32'sh00028f5c;
  localparam signed [31:0] SATURATE_Q = 32'sh01000000;

  // LUT de sigmoide (mesmos valores de sigmoid_lut_pkg.vhd)
  reg [31:0] lut_mem [0:255];
  initial begin
    lut_mem[0] = 32'h000016ad;
    lut_mem[1] = 32'h00001823;
    lut_mem[2] = 32'h000019b1;
    lut_mem[3] = 32'h00001b59;
    lut_mem[4] = 32'h00001d1d;
    lut_mem[5] = 32'h00001efd;
    lut_mem[6] = 32'h000020fd;
    lut_mem[7] = 32'h0000231d;
    lut_mem[8] = 32'h00002560;
    lut_mem[9] = 32'h000027c9;
    lut_mem[10] = 32'h00002a59;
    lut_mem[11] = 32'h00002d14;
    lut_mem[12] = 32'h00002ffc;
    lut_mem[13] = 32'h00003314;
    lut_mem[14] = 32'h0000365e;
    lut_mem[15] = 32'h000039df;
    lut_mem[16] = 32'h00003d9a;
    lut_mem[17] = 32'h00004192;
    lut_mem[18] = 32'h000045cb;
    lut_mem[19] = 32'h00004a4a;
    lut_mem[20] = 32'h00004f13;
    lut_mem[21] = 32'h0000542b;
    lut_mem[22] = 32'h00005997;
    lut_mem[23] = 32'h00005f5c;
    lut_mem[24] = 32'h00006580;
    lut_mem[25] = 32'h00006c09;
    lut_mem[26] = 32'h000072fe;
    lut_mem[27] = 32'h00007a65;
    lut_mem[28] = 32'h00008245;
    lut_mem[29] = 32'h00008aa8;
    lut_mem[30] = 32'h00009394;
    lut_mem[31] = 32'h00009d12;
    lut_mem[32] = 32'h0000a72d;
    lut_mem[33] = 32'h0000b1ee;
    lut_mem[34] = 32'h0000bd5f;
    lut_mem[35] = 32'h0000c98c;
    lut_mem[36] = 32'h0000d681;
    lut_mem[37] = 32'h0000e44a;
    lut_mem[38] = 32'h0000f2f5;
    lut_mem[39] = 32'h00010291;
    lut_mem[40] = 32'h0001132c;
    lut_mem[41] = 32'h000124d7;
    lut_mem[42] = 32'h000137a3;
    lut_mem[43] = 32'h00014ba2;
    lut_mem[44] = 32'h000160e8;
    lut_mem[45] = 32'h0001778a;
    lut_mem[46] = 32'h00018f9c;
    lut_mem[47] = 32'h0001a937;
    lut_mem[48] = 32'h0001c473;
    lut_mem[49] = 32'h0001e16b;
    lut_mem[50] = 32'h00020039;
    lut_mem[51] = 32'h000220fc;
    lut_mem[52] = 32'h000243d2;
    lut_mem[53] = 32'h000268dd;
    lut_mem[54] = 32'h00029040;
    lut_mem[55] = 32'h0002ba1f;
    lut_mem[56] = 32'h0002e6a3;
    lut_mem[57] = 32'h000315f5;
    lut_mem[58] = 32'h00034840;
    lut_mem[59] = 32'h00037db4;
    lut_mem[60] = 32'h0003b682;
    lut_mem[61] = 32'h0003f2dd;
    lut_mem[62] = 32'h000432fd;
    lut_mem[63] = 32'h0004771c;
    lut_mem[64] = 32'h0004bf78;
    lut_mem[65] = 32'h00050c50;
    lut_mem[66] = 32'h00055dea;
    lut_mem[67] = 32'h0005b48c;
    lut_mem[68] = 32'h00061083;
    lut_mem[69] = 32'h0006721f;
    lut_mem[70] = 32'h0006d9b2;
    lut_mem[71] = 32'h00074795;
    lut_mem[72] = 32'h0007bc25;
    lut_mem[73] = 32'h000837c0;
    lut_mem[74] = 32'h0008bace;
    lut_mem[75] = 32'h000945b8;
    lut_mem[76] = 32'h0009d8ec;
    lut_mem[77] = 32'h000a74dd;
    lut_mem[78] = 32'h000b1a05;
    lut_mem[79] = 32'h000bc8e1;
    lut_mem[80] = 32'h000c81f2;
    lut_mem[81] = 32'h000d45bf;
    lut_mem[82] = 32'h000e14d4;
    lut_mem[83] = 32'h000eefc1;
    lut_mem[84] = 32'h000fd71b;
    lut_mem[85] = 32'h0010cb7b;
    lut_mem[86] = 32'h0011cd7d;
    lut_mem[87] = 32'h0012ddc2;
    lut_mem[88] = 32'h0013fcee;
    lut_mem[89] = 32'h00152ba7;
    lut_mem[90] = 32'h00166a95;
    lut_mem[91] = 32'h0017ba63;
    lut_mem[92] = 32'h00191bba;
    lut_mem[93] = 32'h001a8f45;
    lut_mem[94] = 32'h001c15ad;
    lut_mem[95] = 32'h001daf9a;
    lut_mem[96] = 32'h001f5dae;
    lut_mem[97] = 32'h0021208a;
    lut_mem[98] = 32'h0022f8c4;
    lut_mem[99] = 32'h0024e6f0;
    lut_mem[100] = 32'h0026eb96;
    lut_mem[101] = 32'h00290732;
    lut_mem[102] = 32'h002b3a36;
    lut_mem[103] = 32'h002d8503;
    lut_mem[104] = 32'h002fe7ed;
    lut_mem[105] = 32'h00326334;
    lut_mem[106] = 32'h0034f702;
    lut_mem[107] = 32'h0037a36f;
    lut_mem[108] = 32'h003a6877;
    lut_mem[109] = 32'h003d4600;
    lut_mem[110] = 32'h00403bd2;
    lut_mem[111] = 32'h0043499a;
    lut_mem[112] = 32'h00466ee7;
    lut_mem[113] = 32'h0049ab27;
    lut_mem[114] = 32'h004cfdaa;
    lut_mem[115] = 32'h0050659e;
    lut_mem[116] = 32'h0053e212;
    lut_mem[117] = 32'h005771f3;
    lut_mem[118] = 32'h005b1410;
    lut_mem[119] = 32'h005ec717;
    lut_mem[120] = 32'h0062899a;
    lut_mem[121] = 32'h00665a0e;
    lut_mem[122] = 32'h006a36d0;
    lut_mem[123] = 32'h006e1e23;
    lut_mem[124] = 32'h00720e39;
    lut_mem[125] = 32'h00760532;
    lut_mem[126] = 32'h007a0120;
    lut_mem[127] = 32'h007e000b;
    lut_mem[128] = 32'h0081fff5;
    lut_mem[129] = 32'h0085fee0;
    lut_mem[130] = 32'h0089face;
    lut_mem[131] = 32'h008df1c7;
    lut_mem[132] = 32'h0091e1dd;
    lut_mem[133] = 32'h0095c930;
    lut_mem[134] = 32'h0099a5f2;
    lut_mem[135] = 32'h009d7666;
    lut_mem[136] = 32'h00a138e9;
    lut_mem[137] = 32'h00a4ebf0;
    lut_mem[138] = 32'h00a88e0d;
    lut_mem[139] = 32'h00ac1dee;
    lut_mem[140] = 32'h00af9a62;
    lut_mem[141] = 32'h00b30256;
    lut_mem[142] = 32'h00b654d9;
    lut_mem[143] = 32'h00b99119;
    lut_mem[144] = 32'h00bcb666;
    lut_mem[145] = 32'h00bfc42e;
    lut_mem[146] = 32'h00c2ba00;
    lut_mem[147] = 32'h00c59789;
    lut_mem[148] = 32'h00c85c91;
    lut_mem[149] = 32'h00cb08fe;
    lut_mem[150] = 32'h00cd9ccc;
    lut_mem[151] = 32'h00d01813;
    lut_mem[152] = 32'h00d27afd;
    lut_mem[153] = 32'h00d4c5ca;
    lut_mem[154] = 32'h00d6f8ce;
    lut_mem[155] = 32'h00d9146a;
    lut_mem[156] = 32'h00db1910;
    lut_mem[157] = 32'h00dd073c;
    lut_mem[158] = 32'h00dedf76;
    lut_mem[159] = 32'h00e0a252;
    lut_mem[160] = 32'h00e25066;
    lut_mem[161] = 32'h00e3ea53;
    lut_mem[162] = 32'h00e570bb;
    lut_mem[163] = 32'h00e6e446;
    lut_mem[164] = 32'h00e8459d;
    lut_mem[165] = 32'h00e9956b;
    lut_mem[166] = 32'h00ead459;
    lut_mem[167] = 32'h00ec0312;
    lut_mem[168] = 32'h00ed223e;
    lut_mem[169] = 32'h00ee3283;
    lut_mem[170] = 32'h00ef3485;
    lut_mem[171] = 32'h00f028e5;
    lut_mem[172] = 32'h00f1103f;
    lut_mem[173] = 32'h00f1eb2c;
    lut_mem[174] = 32'h00f2ba41;
    lut_mem[175] = 32'h00f37e0e;
    lut_mem[176] = 32'h00f4371f;
    lut_mem[177] = 32'h00f4e5fb;
    lut_mem[178] = 32'h00f58b23;
    lut_mem[179] = 32'h00f62714;
    lut_mem[180] = 32'h00f6ba48;
    lut_mem[181] = 32'h00f74532;
    lut_mem[182] = 32'h00f7c840;
    lut_mem[183] = 32'h00f843db;
    lut_mem[184] = 32'h00f8b86b;
    lut_mem[185] = 32'h00f9264e;
    lut_mem[186] = 32'h00f98de1;
    lut_mem[187] = 32'h00f9ef7d;
    lut_mem[188] = 32'h00fa4b74;
    lut_mem[189] = 32'h00faa216;
    lut_mem[190] = 32'h00faf3b0;
    lut_mem[191] = 32'h00fb4088;
    lut_mem[192] = 32'h00fb88e4;
    lut_mem[193] = 32'h00fbcd03;
    lut_mem[194] = 32'h00fc0d23;
    lut_mem[195] = 32'h00fc497e;
    lut_mem[196] = 32'h00fc824c;
    lut_mem[197] = 32'h00fcb7c0;
    lut_mem[198] = 32'h00fcea0b;
    lut_mem[199] = 32'h00fd195d;
    lut_mem[200] = 32'h00fd45e1;
    lut_mem[201] = 32'h00fd6fc0;
    lut_mem[202] = 32'h00fd9723;
    lut_mem[203] = 32'h00fdbc2e;
    lut_mem[204] = 32'h00fddf04;
    lut_mem[205] = 32'h00fdffc7;
    lut_mem[206] = 32'h00fe1e95;
    lut_mem[207] = 32'h00fe3b8d;
    lut_mem[208] = 32'h00fe56c9;
    lut_mem[209] = 32'h00fe7064;
    lut_mem[210] = 32'h00fe8876;
    lut_mem[211] = 32'h00fe9f18;
    lut_mem[212] = 32'h00feb45e;
    lut_mem[213] = 32'h00fec85d;
    lut_mem[214] = 32'h00fedb29;
    lut_mem[215] = 32'h00feecd4;
    lut_mem[216] = 32'h00fefd6f;
    lut_mem[217] = 32'h00ff0d0b;
    lut_mem[218] = 32'h00ff1bb6;
    lut_mem[219] = 32'h00ff297f;
    lut_mem[220] = 32'h00ff3674;
    lut_mem[221] = 32'h00ff42a1;
    lut_mem[222] = 32'h00ff4e12;
    lut_mem[223] = 32'h00ff58d3;
    lut_mem[224] = 32'h00ff62ee;
    lut_mem[225] = 32'h00ff6c6c;
    lut_mem[226] = 32'h00ff7558;
    lut_mem[227] = 32'h00ff7dbb;
    lut_mem[228] = 32'h00ff859b;
    lut_mem[229] = 32'h00ff8d02;
    lut_mem[230] = 32'h00ff93f7;
    lut_mem[231] = 32'h00ff9a80;
    lut_mem[232] = 32'h00ffa0a4;
    lut_mem[233] = 32'h00ffa669;
    lut_mem[234] = 32'h00ffabd5;
    lut_mem[235] = 32'h00ffb0ed;
    lut_mem[236] = 32'h00ffb5b6;
    lut_mem[237] = 32'h00ffba35;
    lut_mem[238] = 32'h00ffbe6e;
    lut_mem[239] = 32'h00ffc266;
    lut_mem[240] = 32'h00ffc621;
    lut_mem[241] = 32'h00ffc9a2;
    lut_mem[242] = 32'h00ffccec;
    lut_mem[243] = 32'h00ffd004;
    lut_mem[244] = 32'h00ffd2ec;
    lut_mem[245] = 32'h00ffd5a7;
    lut_mem[246] = 32'h00ffd837;
    lut_mem[247] = 32'h00ffdaa0;
    lut_mem[248] = 32'h00ffdce3;
    lut_mem[249] = 32'h00ffdf03;
    lut_mem[250] = 32'h00ffe103;
    lut_mem[251] = 32'h00ffe2e3;
    lut_mem[252] = 32'h00ffe4a7;
    lut_mem[253] = 32'h00ffe64f;
    lut_mem[254] = 32'h00ffe7dd;
    lut_mem[255] = 32'h00ffe953;
  end

  // Multiplicacao Q8.24 com round-to-nearest
  function signed [31:0] qmul;
    input signed [31:0] a;
    input signed [31:0] b;
    reg signed [63:0] p;
    begin
      p = a * b;
      if (p[63]) p = p - 64'sh000000800000;  // +(-2^23)
      else       p = p + 64'sh000000800000;  // +2^23
      p = p >>> 24;
      qmul = p[31:0];
    end
  endfunction

  // Sigmoide via LUT indexada por inteiro
  function signed [31:0] sigmoid_q;
    input signed [31:0] x;
    reg signed [31:0] base;
    integer idx;
    begin
      if (x <= X_MIN_Q) begin
        sigmoid_q = 32'sd1;
      end else if (x >= X_MAX_Q) begin
        sigmoid_q = ONE_Q;
      end else begin
        base = x + X_MAX_Q;
        idx = base >>> 20;
        if (idx > 255) idx = 255;
        sigmoid_q = $signed(lut_mem[idx]);
      end
    end
  endfunction

  wire signed [31:0] one_minus_p = ONE_Q - purity_p;
  wire signed [31:0] term_a      = qmul(ALPHA_Q, one_minus_p);
  wire signed [31:0] term_b      = qmul(BETA_Q, ricci_tilde);
  wire signed [31:0] term_c      = qmul(GAMMA_Q, kolmogorov_cd);
  wire signed [31:0] term_d      = qmul(DELTA_Q, meso);
  wire signed [31:0] cap_raw     = term_a + term_b + term_c + term_d - KAPPA_Q;
  wire signed [31:0] cap         = (cap_raw > 0) ? cap_raw : 32'sd0;
  wire signed [31:0] eta_retro   = sigmoid_q(cap);
  wire signed [31:0] p2          = qmul(purity_p, purity_p);
  wire signed [31:0] opp2        = qmul(one_minus_p, one_minus_p);
  wire signed [31:0] pctc_den    = p2 + opp2;

  wire signed [31:0] delta_p     = purity_p - P_OPERATE_Q;
  wire signed [31:0] arg_h       = qmul(delta_p, INV_SIGMA_P_Q);
  wire signed [31:0] sig_arg_h   = sigmoid_q(arg_h);

  // tau = |n-3|/(n+1)  (numerador/denominador combinacionais)
  wire signed [8:0]  d9       = $signed({1'b0, mode_n}) - 9'sd3;
  wire signed [31:0] abs_d9   = (d9 < 0) ? -$signed({{23{d9[8]}}, d9}) : {{23{d9[8]}}, d9};
  wire signed [31:0] num_tau  = abs_d9;
  wire signed [31:0] den_tau  = $signed({24'd0, mode_n}) + 32'sd1;

  wire        div_valid;
  wire signed [31:0] pctc;
  wire signed [31:0] tau_q;

  qdiv_pipe #(.STAGES(12), .ITERS(2)) div_pctc (
      .clk(clk), .rst_n(rst_n), .valid_in(valid_in),
      .num(p2), .den(pctc_den),
      .valid_out(div_valid), .quot(pctc)
  );

  qdiv_pipe #(.STAGES(12), .ITERS(2)) div_tau (
      .clk(clk), .rst_n(rst_n), .valid_in(valid_in),
      .num(num_tau), .den(den_tau),
      .valid_out(), .quot(tau_q)
  );

  // Linhas de atraso: DIV_STAGES+1 registros (casam com a latencia do divisor)
  reg signed [31:0] cap_d [0:12];
  reg signed [31:0] eta_d [0:12];
  reg signed [31:0] sgh_d [0:12];
  reg [12:0] v_d;
  integer di;

  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      v_d <= 0;
      for (di = 0; di <= 12; di = di + 1) begin
        cap_d[di] <= 32'sd0;
        eta_d[di] <= 32'sd0;
        sgh_d[di] <= 32'sd0;
      end
    end else begin
      cap_d[0] <= cap;
      eta_d[0] <= eta_retro;
      sgh_d[0] <= sig_arg_h;
      v_d <= {v_d[11:0], valid_in};
      for (di = 0; di < 12; di = di + 1) begin
        cap_d[di + 1] <= cap_d[di];
        eta_d[di + 1] <= eta_d[di];
        sgh_d[di + 1] <= sgh_d[di];
      end
    end
  end

  wire signed [31:0] eff        = qmul(eta_d[12], pctc);
  wire signed [31:0] h_factor   = qmul(tau_q, INV_TAU_CRIT_Q);
  wire signed [31:0] horizon_i  = qmul(h_factor, sgh_d[12]);

  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      retro_capacity <= 32'sd0;
      retro_eff      <= 32'sd0;
      pctc_prob      <= 32'sd0;
      eff_eff        <= 32'sd0;
      torsion        <= 32'sd0;
      horizon        <= 32'sd0;
    end else begin
      retro_capacity <= cap_d[12];
      retro_eff      <= eta_d[12];
      pctc_prob      <= pctc;
      eff_eff        <= eff;
      torsion        <= tau_q;
      horizon        <= horizon_i;
      valid_out      <= v_d[12];
    end
  end

  assign ctc_flag      = (eff_eff   > HALF_Q)           ? 1'b1 : 1'b0;
  assign retro_active  = (retro_capacity > RETRO_ON_Q)  ? 1'b1 : 1'b0;
  assign horizon_alarm = (horizon   > HORIZON_ABORT_Q)  ? 1'b1 : 1'b0;

endmodule
`default_nettype wire
