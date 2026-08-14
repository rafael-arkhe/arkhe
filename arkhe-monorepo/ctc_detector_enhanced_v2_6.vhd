-- ============================================================
-- ctc_detector_enhanced_v2_6.vhd
-- Detector de Curvas de Tempo Fechado (CTC) — BLOCK 11 v2.6
-- Arithmetica em ponto fixo Q8.24 (signed 32) / LUT sigmoide.
--
-- Fundamentos (validacao cruzada 2026-08-11):
--   1. Capacidade retrocausal one-shot (Ji, Lloyd & Wilde, PRL 2026):
--        C_retro = max(0, alfa*(1-p) + beta*R~ + gama*CD + delta*Meso - kappa)
--   2. Probabilidade de sucesso da PCTC (Huang et al., 2026):
--        P_PCTC = p^2 / (p^2 + (1-p)^2)
--   3. Eficiencia efetiva de retrocomunicacao:
--        eta_efetivo = sigmoide(C_retro) * P_PCTC
--   4. Indice de horizonte de cronologia (Xavier, 2026):
--        H = (tau/tau_crit) * sigmoide((p - p_operate)/sigma_p)
--        tau = |n-3|/(n+1)   (modo fundamental n=3)
--
-- Convencoes:
--   - Fixo: Q8.24 (8 bits inteiros, 24 fracionarios); 1.0 = x"01000000".
--   - Divisao: pipeline de 12 estagios (qdiv_pipe), 24 iteracoes de
--     resto-restaurador em 2 iteracoes/estagio. Latencia total
--     DIV_STAGES+1 ciclos entre valid_in e valid_out. Satura em 1.0
--     para quocientes >= 1.0 (dominio das metricas e [0,1]).
--   - Sigmoide: LUT de 256 entradas de sigmoid_lut_pkg (dominio [-8,8]),
--     indexada por inteiro (sintetizavel, sem real).
--   - Saidas registradas em clk (timing); flags combinacionais sobre as
--     saidas registradas.
--
-- Handover do KSM:
--   ctc_flag       = '1' quando eta_efetivo > 0.5
--   retro_active   = '1' quando C_retro > 0.01
--   horizon_alarm  = '1' quando H > 0.7  (abortar ignicao)
-- ============================================================

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.sigmoid_lut_pkg.all;

entity ctc_detector_enhanced_v2_6 is
    generic (
        ALPHA_Q          : signed(31 downto 0) := x"00666666";  -- 0.4000
        BETA_Q           : signed(31 downto 0) := x"004CCCCD";  -- 0.3000
        GAMMA_Q          : signed(31 downto 0) := x"00333333";  -- 0.2000
        DELTA_Q          : signed(31 downto 0) := x"0019999A";  -- 0.1000
        KAPPA_Q          : signed(31 downto 0) := x"00028F5C";  -- 0.0100
        P_OPERATE_Q      : signed(31 downto 0) := x"0007D56A";  -- 0.0306
        INV_SIGMA_P_Q    : signed(31 downto 0) := x"64000000";  -- 1/0.0100
        INV_TAU_CRIT_Q   : signed(31 downto 0) := x"06AAAAAB";  -- 1/0.1500
        HALF_Q           : signed(31 downto 0) := x"00800000";  -- 0.5000
        HORIZON_ABORT_Q  : signed(31 downto 0) := x"00B33333";  -- 0.7000
        RETRO_ON_Q       : signed(31 downto 0) := x"00028F5C";  -- 0.0100
        SATURATE_Q       : signed(31 downto 0) := x"01000000";  -- 1.0000
        DIV_STAGES       : positive := 12                       -- estagios do divisor
    );
    port (
        clk            : in  std_logic;
        rst_n          : in  std_logic;
        valid_in       : in  std_logic;

        -- Metricas espectrais (provenientes do bloco Spectral Metrics)
        purity_p       : in  signed(31 downto 0);  -- pureza espectral p
        ricci_tilde    : in  signed(31 downto 0);  -- curvatura de Ricci R~
        kolmogorov_cd  : in  signed(31 downto 0);  -- complexidade de Kolmogorov
        meso           : in  signed(31 downto 0);  -- mesoestrutura
        mode_n         : in  unsigned(7 downto 0); -- modo dominante n

        valid_out      : out std_logic;
        retro_capacity : out signed(31 downto 0) := (others => '0');  -- C_retro^(1)
        retro_eff      : out signed(31 downto 0) := (others => '0');  -- eta_retro
        pctc_prob      : out signed(31 downto 0) := (others => '0');  -- P_PCTC
        eff_eff        : out signed(31 downto 0) := (others => '0');  -- eta_efetivo
        torsion        : out signed(31 downto 0) := (others => '0');  -- tau
        horizon        : out signed(31 downto 0) := (others => '0');  -- H
        ctc_flag       : out std_logic := '0';
        retro_active   : out std_logic := '0';
        horizon_alarm  : out std_logic := '0'
    );
end entity ctc_detector_enhanced_v2_6;

architecture rtl of ctc_detector_enhanced_v2_6 is

    constant ONE_Q   : signed(31 downto 0) := x"01000000";
    constant X_MIN_Q : signed(31 downto 0) := to_signed(-2**27, 32);  -- -8.0
    constant X_MAX_Q : signed(31 downto 0) := to_signed(2**27, 32);   -- +8.0

    ------------------------------------------------------------------
    -- Multiplicacao Q8.24 com arredondamento (round-to-nearest)
    ------------------------------------------------------------------
    function qmul(a : signed(31 downto 0);
                  b : signed(31 downto 0)) return signed is
        variable p : signed(63 downto 0);
        variable r : signed(63 downto 0);
    begin
        p := a * b;
        if p(63) = '1' then
            r := p + to_signed(-(2**23), 64);
        else
            r := p + to_signed(2**23, 64);
        end if;
        r := shift_right(r, 24);
        return r(31 downto 0);
    end function qmul;

    ------------------------------------------------------------------
    -- Sigmoide via LUT indexada por inteiro (sem real, sintetizavel).
    -- idx = (x + 8*2^24) / 2^20, pois passo = 0.0625 = 2^20 em Q8.24.
    ------------------------------------------------------------------
    function sigmoid_q(x : signed(31 downto 0)) return signed is
        variable base : signed(31 downto 0);
        variable idx  : integer range 0 to SIGMOID_LUT_SIZE - 1;
    begin
        if x <= X_MIN_Q then
            return to_signed(1, 32);              -- sigma ~ 0
        elsif x >= X_MAX_Q then
            return ONE_Q;                         -- sigma = 1.0
        else
            base := x + X_MAX_Q;                  -- x + 8*2^24
            idx := to_integer(shift_right(base, 20));
            if idx > SIGMOID_LUT_SIZE - 1 then
                idx := SIGMOID_LUT_SIZE - 1;
            end if;
            return signed(SIGMOID_LUT(idx));
        end if;
    end function sigmoid_q;

    type s32_arr is array (natural range <>) of signed(31 downto 0);

    -- Datapath combinacional (ciclo da entrada)
    signal one_minus_p : signed(31 downto 0) := (others => '0');
    signal term_a      : signed(31 downto 0) := (others => '0');
    signal term_b      : signed(31 downto 0) := (others => '0');
    signal term_c      : signed(31 downto 0) := (others => '0');
    signal term_d      : signed(31 downto 0) := (others => '0');
    signal cap_raw     : signed(31 downto 0) := (others => '0');
    signal cap         : signed(31 downto 0) := (others => '0');
    signal eta_retro   : signed(31 downto 0) := (others => '0');
    signal p2          : signed(31 downto 0) := (others => '0');
    signal opp2        : signed(31 downto 0) := (others => '0');
    signal pctc_den    : signed(31 downto 0) := (others => '0');
    signal delta_p     : signed(31 downto 0) := (others => '0');
    signal arg_h       : signed(31 downto 0) := (others => '0');
    signal sig_arg_h   : signed(31 downto 0) := (others => '0');
    signal d9          : signed(8 downto 0)  := (others => '0');
    signal num_tau     : signed(31 downto 0) := (others => '0');
    signal den_tau     : signed(31 downto 0) := (others => '0');

    -- Divisores pipelineados
    signal pctc      : signed(31 downto 0) := (others => '0');
    signal tau_q     : signed(31 downto 0) := (others => '0');
    signal div_valid : std_logic := '0';

    -- Linhas de atraso para alinhar com a latencia do divisor
    signal cap_d : s32_arr(0 to DIV_STAGES);
    signal eta_d : s32_arr(0 to DIV_STAGES);
    signal sgh_d : s32_arr(0 to DIV_STAGES);

    -- Pos-divisor (ciclo DIV_STAGES)
    signal eff        : signed(31 downto 0) := (others => '0');
    signal h_factor   : signed(31 downto 0) := (others => '0');
    signal horizon_i  : signed(31 downto 0) := (others => '0');

    signal valid_d : std_logic := '0';

begin

    ------------------------------------------------------------------
    -- Datapath combinacional a partir das entradas
    ------------------------------------------------------------------
    one_minus_p <= ONE_Q - purity_p;

    term_a <= qmul(ALPHA_Q, one_minus_p);
    term_b <= qmul(BETA_Q, ricci_tilde);
    term_c <= qmul(GAMMA_Q, kolmogorov_cd);
    term_d <= qmul(DELTA_Q, meso);

    cap_raw <= term_a + term_b + term_c + term_d - KAPPA_Q;
    cap     <= cap_raw when cap_raw > 0 else (others => '0');

    eta_retro <= sigmoid_q(cap);

    p2        <= qmul(purity_p, purity_p);
    opp2      <= qmul(one_minus_p, one_minus_p);
    pctc_den  <= p2 + opp2;

    delta_p   <= purity_p - P_OPERATE_Q;
    arg_h     <= qmul(delta_p, INV_SIGMA_P_Q);
    sig_arg_h <= sigmoid_q(arg_h);

    -- Numerador/denominador da torcao: |n-3| e (n+1)
    d9      <= signed(resize(mode_n, 9)) - to_signed(3, 9);
    num_tau <= resize(-d9, 32) when d9 < 0 else resize(d9, 32);
    den_tau <= resize(signed(resize(mode_n, 32)) + 1, 32);

    ------------------------------------------------------------------
    -- Divisores pipelineados (latencia DIV_STAGES)
    ------------------------------------------------------------------
    div_pctc : entity work.qdiv_pipe
        generic map (
            STAGES => DIV_STAGES,
            ITERS  => 24 / DIV_STAGES
        )
        port map (
            clk       => clk,
            rst_n     => rst_n,
            valid_in  => valid_in,
            num       => p2,
            den       => pctc_den,
            valid_out => div_valid,
            quot      => pctc
        );

    div_tau : entity work.qdiv_pipe
        generic map (
            STAGES => DIV_STAGES,
            ITERS  => 24 / DIV_STAGES
        )
        port map (
            clk       => clk,
            rst_n     => rst_n,
            valid_in  => valid_in,
            num       => num_tau,
            den       => den_tau,
            valid_out => open,
            quot      => tau_q
        );

    ------------------------------------------------------------------
    -- Linhas de atraso (alinhamento com a latencia do divisor).
    -- Registros reais: cap_d(0) registra cap; a cadeia (1..DIV_STAGES)
    -- desloca um registro por ciclo. Total de DIV_STAGES+1 registros,
    -- casando com os DIV_STAGES estagios do divisor + registro de saida.
    ------------------------------------------------------------------
    proc_delay0 : process (clk, rst_n)
    begin
        if rst_n = '0' then
            cap_d(0) <= (others => '0');
            eta_d(0) <= (others => '0');
            sgh_d(0) <= (others => '0');
        elsif rising_edge(clk) then
            cap_d(0) <= cap;
            eta_d(0) <= eta_retro;
            sgh_d(0) <= sig_arg_h;
        end if;
    end process proc_delay0;

    gen_delay : for i in 0 to DIV_STAGES - 1 generate
        process (clk, rst_n)
        begin
            if rst_n = '0' then
                cap_d(i + 1) <= (others => '0');
                eta_d(i + 1) <= (others => '0');
                sgh_d(i + 1) <= (others => '0');
            elsif rising_edge(clk) then
                cap_d(i + 1) <= cap_d(i);
                eta_d(i + 1) <= eta_d(i);
                sgh_d(i + 1) <= sgh_d(i);
            end if;
        end process;
    end generate gen_delay;

    ------------------------------------------------------------------
    -- Combinacional pos-divisor (ciclo DIV_STAGES)
    ------------------------------------------------------------------
    eff       <= qmul(eta_d(DIV_STAGES), pctc);
    h_factor  <= qmul(tau_q, INV_TAU_CRIT_Q);
    horizon_i <= qmul(h_factor, sgh_d(DIV_STAGES));

    ------------------------------------------------------------------
    -- Registro de saida (timing closure) + flags
    ------------------------------------------------------------------
    process (clk, rst_n)
    begin
        if rst_n = '0' then
            valid_d        <= '0';
            retro_capacity <= (others => '0');
            retro_eff      <= (others => '0');
            pctc_prob      <= (others => '0');
            eff_eff        <= (others => '0');
            torsion        <= (others => '0');
            horizon        <= (others => '0');
        elsif rising_edge(clk) then
            valid_d        <= div_valid;
            retro_capacity <= cap_d(DIV_STAGES);
            retro_eff      <= eta_d(DIV_STAGES);
            pctc_prob      <= pctc;
            eff_eff        <= eff;
            torsion        <= tau_q;
            horizon        <= horizon_i;
        end if;
    end process;

    valid_out     <= valid_d;
    ctc_flag      <= '1' when eff_eff   > HALF_Q          else '0';
    retro_active  <= '1' when retro_capacity > RETRO_ON_Q else '0';
    horizon_alarm <= '1' when horizon   > HORIZON_ABORT_Q else '0';

end architecture rtl;
