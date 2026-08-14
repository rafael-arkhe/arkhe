# arkhe_trader_v3.1_final.py
# Código completo com todas as correções (exponential map, distância hiperbólica, custos, decaimento)
# Salve e execute. Os resultados serão a verdade empírica.

import numpy as np
import yfinance as yf
from scipy.stats import ttest_1samp
import pandas as pd
from datetime import datetime, timedelta
import warnings
warnings.filterwarnings('ignore')

# ------------------------------------------------------------
# 1. GEOMETRIA HIPERBÓLICA CORRIGIDA
# ------------------------------------------------------------
def poincare_dist(z1: complex, z2: complex) -> float:
    num = abs(z1 - z2)**2
    den = (1 - abs(z1)**2) * (1 - abs(z2)**2)
    val = 1 + 2 * num / (den + 1e-12)
    return np.arccosh(np.clip(val, 1.0, 1e12))

def mobius_add(x: np.ndarray, y: np.ndarray) -> np.ndarray:
    x_norm = np.linalg.norm(x)**2
    y_norm = np.linalg.norm(y)**2
    xy_dot = np.dot(x, y)
    num = (1 + 2*xy_dot + y_norm) * x + (1 - x_norm) * y
    den = 1 + 2*xy_dot + x_norm * y_norm
    return num / den

def exponential_map(x: np.ndarray, v: np.ndarray) -> np.ndarray:
    v_norm = np.linalg.norm(v)
    if v_norm < 1e-8:
        return x
    lambda_x = 2 / (1 - np.linalg.norm(x)**2)
    direction = v / v_norm
    move_scale = np.tanh((lambda_x * v_norm) / 2)
    move = move_scale * direction
    return mobius_add(x, move)

def hyperbolic_gradient(potential_fn, z: complex, eps: float = 1e-6) -> complex:
    dV_dx = (potential_fn(z + eps) - potential_fn(z - eps)) / (2*eps)
    dV_dy = (potential_fn(z + eps*1j) - potential_fn(z - eps*1j)) / (2*eps)
    grad_e = complex(dV_dx, dV_dy)
    scale = ((1 - abs(z)**2) ** 2) / 4
    return scale * grad_e

def step_flow(z: complex, potential_fn, step_size: float = 0.05) -> complex:
    grad = hyperbolic_gradient(potential_fn, z)
    if abs(grad) < 1e-12:
        return z
    x = np.array([z.real, z.imag])
    v = np.array([grad.real, grad.imag]) * step_size
    new_x = exponential_map(x, v)
    norm = np.linalg.norm(new_x)
    if norm >= 0.99:
        new_x = new_x / norm * 0.98
    return complex(new_x[0], new_x[1])

# ------------------------------------------------------------
# 2. MARKET MAPPER (HEURÍSTICO, SEM FISHER-RAO)
# ------------------------------------------------------------
class MarketMapper:
    def __init__(self, lookback=20):
        self.lookback = lookback
    def map_asset(self, prices: np.ndarray, all_vols: list = None) -> complex:
        returns = np.diff(np.log(prices))[-self.lookback:]
        if len(returns) < 5:
            return complex(0.0, 0.0)
        vol = np.std(returns)
        if all_vols is not None and len(all_vols) > 1:
            rank = np.searchsorted(np.sort(all_vols), vol) / len(all_vols)
            r = 0.1 + 0.8 * rank
        else:
            r = np.tanh(vol / (np.median(all_vols) * 2)) if all_vols else 0.1
        r = np.clip(r, 0.01, 0.98)
        x = np.arange(self.lookback)
        slope, _ = np.polyfit(x, prices[-self.lookback:], 1)
        daily_return_approx = slope / np.mean(prices[-self.lookback:])
        norm_slope = daily_return_approx / (vol + 1e-6)
        theta = np.arctan(np.clip(norm_slope, -5, 5)) % (2 * np.pi)
        return complex(r * np.cos(theta), r * np.sin(theta))

# ------------------------------------------------------------
# 3. GEOMETRIA ADAPTATIVA (COM DECAIMENTO E DISTÂNCIA HIPERBÓLICA)
# ------------------------------------------------------------
class AdaptiveGeometry:
    def __init__(self, max_points=15, half_life=60, min_t_stat=2.0):
        self.max_points = max_points
        self.decay = np.log(2) / half_life
        self.min_t_stat = min_t_stat
        self.zeros = []  # (z, weight, timestamp)
        self.poles = []
        self.step = 0

    def update(self, z: complex, recent_pnl: list):
        self.step += 1
        t = self.step
        self.zeros = [(z0, w * np.exp(-self.decay * (t - ts)), ts)
                      for z0, w, ts in self.zeros if w * np.exp(-self.decay * (t - ts)) > 0.01]
        self.poles = [(p, w * np.exp(-self.decay * (t - ts)), ts)
                      for p, w, ts in self.poles if w * np.exp(-self.decay * (t - ts)) > 0.01]
        if len(recent_pnl) >= 20:
            t_stat, p_val = ttest_1samp(recent_pnl, 0)
            mean_pnl = np.mean(recent_pnl)
            if t_stat > self.min_t_stat and p_val < 0.05:
                weight = min(abs(mean_pnl) * 10, 0.5)
                if not any(poincare_dist(z, z0) < 0.15 for z0, _, _ in self.zeros):
                    self.zeros.append((z, weight, t))
            elif t_stat < -self.min_t_stat and p_val < 0.05:
                weight = min(abs(mean_pnl) * 10, 0.5)
                if not any(poincare_dist(z, p0) < 0.15 for p0, _, _ in self.poles):
                    self.poles.append((z, weight, t))
        self.zeros = sorted(self.zeros, key=lambda x: x[1], reverse=True)[:self.max_points]
        self.poles = sorted(self.poles, key=lambda x: x[1], reverse=True)[:self.max_points]

    def potential(self, z: complex) -> float:
        val = 0.0
        for z0, w, _ in self.zeros:
            d = poincare_dist(z, z0)
            val += -w * np.log(d + 0.01)
        for p, w, _ in self.poles:
            d = poincare_dist(z, p)
            val += w * np.log(d + 0.01)
        return val

class RiskManager:
    def __init__(self, max_drawdown=0.12):
        self.peak = 1.0
        self.max_dd = max_drawdown
    def check(self, equity: float) -> str:
        self.peak = max(self.peak, equity)
        dd = (self.peak - equity) / self.peak
        return "EMERGENCY_LIQUIDATION" if dd > self.max_dd else "NORMAL"

# ------------------------------------------------------------
# 4. TRADER PRINCIPAL
# ------------------------------------------------------------
class ArkheTrader:
    def __init__(self, symbols, lookback=20, risk_limit=0.8, max_drawdown=0.12, warmup_days=20):
        self.symbols = symbols
        self.lookback = lookback
        self.risk_limit = risk_limit
        self.warmup_days = warmup_days
        self.warmup_counter = 0

        self.mapper = MarketMapper(lookback)
        self.geometry = AdaptiveGeometry()
        self.risk = RiskManager(max_drawdown)

        self.positions = {s: 0.0 for s in symbols}
        self.states = {s: complex(0,0) for s in symbols}
        self.equity = 1.0
        self.history = []
        self.pnl_history = {s: [] for s in symbols}

        # Para o momentum durante o warmup
        self.recent_returns = {s: 0.0 for s in symbols}

    def update_data(self, prices_dict: dict):
        all_vols = []
        for sym, prices in prices_dict.items():
            if len(prices) >= self.lookback:
                returns = np.diff(np.log(prices))[-self.lookback:]
                all_vols.append(np.std(returns))
                # Armazena o retorno acumulado para o warmup
                self.recent_returns[sym] = np.mean(returns) * self.lookback
        for sym, prices in prices_dict.items():
            if len(prices) >= self.lookback:
                self.states[sym] = self.mapper.map_asset(prices, all_vols)

    def decide(self) -> dict:
        new_positions = {}
        total_risk = 0.0

        # --- FASE DE WARMUP (momentum puro) ---
        if self.warmup_counter < self.warmup_days:
            for sym in self.symbols:
                # Momentum simples: retorno acumulado / volatilidade
                vol = np.std(self.pnl_history[sym][-20:]) if len(self.pnl_history[sym]) >= 10 else 0.02
                raw_weight = self.recent_returns[sym] / (vol + 1e-6)
                weight = np.clip(raw_weight * 0.15, -0.3, 0.3)
                new_positions[sym] = weight
                total_risk += abs(weight)
            self.warmup_counter += 1

        # --- FASE HIPERBÓLICA (após warmup) ---
        else:
            for sym, z in self.states.items():
                z_cur = z
                for _ in range(3):
                    z_cur = step_flow(z_cur, self.geometry.potential, step_size=0.02)

                pot_diff = self.geometry.potential(z_cur) - self.geometry.potential(z)
                weight = np.clip(pot_diff * 2.0, -0.3, 0.3)
                new_positions[sym] = weight
                total_risk += abs(weight)

        # Escalonamento de risco
        if total_risk > self.risk_limit:
            scale = self.risk_limit / total_risk
            new_positions = {s: w * scale for s, w in new_positions.items()}

        return new_positions

    def update_equity(self, returns: dict, decisions: dict):
        # 1. PnL bruto com as posições ATUAIS (anteriores à decisão)
        gross_pnl = sum(self.positions[s] * returns[s] for s in self.symbols)

        # 2. Custo de transação (0.15% do turnover)
        turnover = sum(abs(decisions[s] - self.positions[s]) for s in self.symbols)
        cost = turnover * 0.0015
        net_pnl = gross_pnl - cost

        # 3. Atualiza equity
        self.equity *= (1 + net_pnl)

        # 4. ARMAZENA PnL por ativo ANTES de atualizar posições (corrige o bug)
        for s in self.symbols:
            if s in returns:
                self.pnl_history[s].append(self.positions[s] * returns[s])
                if len(self.pnl_history[s]) > 60:
                    self.pnl_history[s].pop(0)

        # 5. Atualiza posições APÓS o PnL
        self.positions = decisions

        # 6. Atualiza geometria (apenas após o warmup)
        if self.warmup_counter >= self.warmup_days and len(self.history) % 5 == 0:
            for s in self.symbols:
                if len(self.pnl_history[s]) >= 20:
                    self.geometry.update(self.states[s], self.pnl_history[s])

        self.history.append(self.equity)

# ------------------------------------------------------------
# 5. EXECUÇÃO DO BACKTEST
# ------------------------------------------------------------
if __name__ == "__main__":
    symbols = ['AAPL', 'MSFT', 'GOOG', 'AMZN', 'TSLA']
    end = datetime.now()
    start = end - timedelta(days=3*365)
    print("Downloading data...")
    data = yf.download(symbols, start=start, end=end, interval='1d', auto_adjust=False, progress=False)['Adj Close'].dropna()
    if len(data) < 62:
        raise RuntimeError(f"Poucos dados baixados: {len(data)} linhas")
    trader = ArkheTrader(symbols, lookback=20, risk_limit=0.8, max_drawdown=0.12)
    dates = data.index
    print("Running backtest...")
    for i in range(60, len(dates) - 1):
        current_prices = {s: data[s].iloc[:i+1].values for s in symbols}
        trader.update_data(current_prices)
        decisions = trader.decide()
        next_prices = {s: data[s].iloc[i+1] for s in symbols}
        curr_prices = {s: data[s].iloc[i] for s in symbols}
        returns = {s: (next_prices[s] - curr_prices[s]) / curr_prices[s] for s in symbols}
        trader.update_equity(returns, decisions)
        if trader.risk.check(trader.equity) == "EMERGENCY_LIQUIDATION":
            print(f"KaliYuga em {dates[i].date()}")
            break
    # Métricas
    eq = pd.Series(trader.history, index=dates[60:len(trader.history)+60])
    rets = eq.pct_change().dropna()
    ann_ret = rets.mean() * 252
    ann_vol = rets.std() * np.sqrt(252)
    sharpe = ann_ret / ann_vol if ann_vol > 0 else 0
    max_dd = (eq / eq.expanding().max() - 1).min()
    print("\n" + "="*50)
    print("ARKHE TRADER v3.1 - RELATORIO FINAL")
    print("="*50)
    print(f"Periodo: {dates[60].date()} a {dates[-1].date()}")
    print(f"Retorno Anualizado: {ann_ret*100:.2f}%")
    print(f"Volatilidade Anual:  {ann_vol*100:.2f}%")
    print(f"Sharpe Ratio:        {sharpe:.2f}")
    print(f"Max Drawdown:        {max_dd*100:.2f}%")
    print(f"Equity Final:        {eq.iloc[-1]:.4f}")
    print(f"Zeros Ativos:        {len(trader.geometry.zeros)}")
    print(f"Polos Ativos:        {len(trader.geometry.poles)}")
    print("="*50)
