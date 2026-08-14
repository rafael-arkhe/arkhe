-- ============================================================
-- qdiv_pipe.vhd
-- Divisor Q8.24 pipelineado (resto-restaurador em estagios).
-- BLOCK 11 v2.6 -- reduz o caminho critico combinacional para
-- fechar timing a 100 MHz no VE2602 (mitigacao R8).
--
-- 24 iteracoes de divisao divididas em STAGES estagios de
-- pipeline, cada um executando ITERS iteracoes (STAGES*ITERS=24).
-- Dominio: |num| <= |den| (quociente em [0,1]); satura em 1.0
-- (SATURATE_Q) quando |num| >= |den| ou den = 0.
-- Latencia: STAGES ciclos de clk entre valid_in e valid_out.
-- ============================================================

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity qdiv_pipe is
    generic (
        STAGES : positive := 12;   -- numero de estagios de pipeline
        ITERS  : positive := 2     -- iteracoes por estagio (24/STAGES)
    );
    port (
        clk       : in  std_logic;
        rst_n     : in  std_logic;
        valid_in  : in  std_logic;
        num       : in  signed(31 downto 0);
        den       : in  signed(31 downto 0);
        valid_out : out std_logic;
        quot      : out signed(31 downto 0)
    );
end entity qdiv_pipe;

architecture rtl of qdiv_pipe is

    constant SATURATE_Q : signed(31 downto 0) := x"01000000";  -- 1.0 em Q8.24

    type u33_arr is array (natural range <>) of unsigned(32 downto 0);
    type u32_arr is array (natural range <>) of unsigned(31 downto 0);

    -- Iteracoes de resto-restaurador com acumulacao do quociente.
    -- first_bit: posicao do bit do quociente produzido pela 1a iteracao.
    procedure restore_iters (
        r         : inout unsigned(32 downto 0);
        q         : inout unsigned(31 downto 0);
        b         : in    unsigned(32 downto 0);
        first_bit : in    natural;
        niter     : in    natural
    ) is
    begin
        for i in 0 to niter - 1 loop
            r := r sll 1;
            if r >= b then
                r := r - b;
                q(first_bit - i) := '1';
            end if;
        end loop;
    end procedure restore_iters;

    signal r_pipe : u33_arr(0 to STAGES);
    signal q_pipe : u32_arr(0 to STAGES);
    signal b_pipe : u33_arr(0 to STAGES);
    signal n_pipe : std_logic_vector(0 to STAGES);  -- sinal (negativo)
    signal s_pipe : std_logic_vector(0 to STAGES);  -- saturacao
    signal v_pipe : std_logic_vector(0 to STAGES);  -- valid

    signal quot_i : signed(31 downto 0) := (others => '0');

begin

    process (clk, rst_n)
        variable A   : unsigned(31 downto 0);
        variable B   : unsigned(31 downto 0);
        variable rv  : unsigned(32 downto 0);
        variable qv  : unsigned(31 downto 0);
    begin
        if rst_n = '0' then
            for i in 0 to STAGES loop
                r_pipe(i) <= (others => '0');
                q_pipe(i) <= (others => '0');
                b_pipe(i) <= (others => '0');
                n_pipe(i) <= '0';
                s_pipe(i) <= '0';
                v_pipe(i) <= '0';
            end loop;
        elsif rising_edge(clk) then
            -- Avanca o valid/sign/sat pelo pipeline
            for i in 0 to STAGES - 1 loop
                v_pipe(i + 1) <= v_pipe(i);
                n_pipe(i + 1) <= n_pipe(i);
                s_pipe(i + 1) <= s_pipe(i);
            end loop;

            -- Estagios de divisao
            for i in 0 to STAGES - 1 loop
                rv := r_pipe(i);
                qv := q_pipe(i);
                restore_iters(rv, qv, b_pipe(i), 23 - i * ITERS, ITERS);
                r_pipe(i + 1) <= rv;
                q_pipe(i + 1) <= qv;
                b_pipe(i + 1) <= b_pipe(i);
            end loop;

            -- Captura de entrada
            if valid_in = '1' then
                A := unsigned(num) when num(31) = '0' else unsigned(-num);
                B := unsigned(den) when den(31) = '0' else unsigned(-den);
                b_pipe(0) <= resize(B, 33);
                r_pipe(0) <= resize(A, 33);
                q_pipe(0) <= (others => '0');
                n_pipe(0) <= num(31) xor den(31);
                s_pipe(0) <= '1' when (den = 0 or A >= B) else '0';
                v_pipe(0) <= '1';
            else
                v_pipe(0) <= '0';
            end if;
        end if;
    end process;

    -- Saida combinacional sobre o ultimo estagio (alinhada ao valid)
    quot_i <= SATURATE_Q when s_pipe(STAGES) = '1'
              else signed(q_pipe(STAGES));
    quot      <= -quot_i when n_pipe(STAGES) = '1' else quot_i;
    valid_out <= v_pipe(STAGES);

end architecture rtl;
