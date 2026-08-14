import numpy as np
import pandas as pd
import yfinance as yf
from datetime import datetime, timedelta

class ArkheCore:
    """Nucleo funcional do ARKHE TRADER (sem geometria)."""

    def __init__(self, symbols, lookback=20, risk_limit=0.8, max_drawdown=0.12):
        self.symbols = symbols
        self.lookback = lookback
        self.risk_limit = risk_limit
        self.max_drawdown = max_drawdown
        self.positions = {s: 0.0 for s in symbols}
        self.old_positions = {s: 0.0 for s in symbols}
        self.equity = 1.0
        self.peak_equity = 1.0
        self.history = []

    def compute_signal(self, prices_dict):
        all_vols = []
        for s, prices in prices_dict.items():
            rets = np.diff(np.log(prices))[-self.lookback:]
            all_vols.append(np.std(rets))

        signals = {}
        for s, prices in prices_dict.items():
            rets = np.diff(np.log(prices))[-self.lookback:]
            vol = np.std(rets) + 1e-12
            rank = np.searchsorted(np.sort(all_vols), vol) / len(all_vols)
            r = np.clip(0.1 + 0.8 * rank, 0.01, 0.98)
            slope, _ = np.polyfit(np.arange(self.lookback), prices[-self.lookback:], 1)
            momentum = slope / np.mean(prices[-self.lookback:])
            signals[s] = r * np.clip(momentum / vol, -1, 1)
        return signals

    def step(self, prices_dict):
        signals = self.compute_signal(prices_dict)
        total = sum(abs(w) for w in signals.values())
        if total > self.risk_limit:
            scale = self.risk_limit / total
            signals = {s: w * scale for s, w in signals.items()}
        self.old_positions = self.positions.copy()
        self.positions = signals
        return signals

    def update_equity(self, returns, cost=0.0015):
        old_pos = self.old_positions
        turnover = sum(abs(self.positions[s] - old_pos.get(s, 0)) for s in self.symbols)
        gross_pnl = sum(old_pos.get(s, 0) * returns.get(s, 0) for s in self.symbols)
        net_pnl = gross_pnl - turnover * cost
        self.equity *= (1 + net_pnl)
        self.peak_equity = max(self.peak_equity, self.equity)
        self.history.append(self.equity)
        return (self.peak_equity - self.equity) / self.peak_equity > self.max_drawdown


def metrics(eq_series):
    rets = eq_series.pct_change().dropna()
    if len(rets) == 0 or rets.std() == 0:
        return 0.0, 0.0, 0.0, 0.0
    ar = rets.mean() * 252
    av = rets.std() * np.sqrt(252)
    sh = ar / av if av > 0 else 0
    dd = (eq_series / eq_series.expanding().max() - 1).min()
    return ar, av, sh, dd


if __name__ == "__main__":
    symbols = ['AAPL', 'MSFT', 'GOOG', 'AMZN', 'TSLA']
    end = datetime.now()
    start = end - timedelta(days=3 * 365)
    print("Baixando dados reais...")
    data = yf.download(symbols, start=start, end=end, interval='1d', auto_adjust=False, progress=False)['Adj Close'].dropna()
    dates = data.index

    trader = ArkheCore(symbols)
    stopped = False
    for i in range(60, len(data) - 1):
        prices = {s: data[s].iloc[:i + 1].values for s in symbols}
        returns = {s: (data[s].iloc[i + 1] - data[s].iloc[i]) / data[s].iloc[i] for s in symbols}
        trader.step(prices)
        if trader.update_equity(returns):
            print(f"Stop loss acionado em {dates[i].date()}")
            stopped = True
            break

    eq_idx = dates[60:len(trader.history) + 60]
    eq = pd.Series(trader.history, index=eq_idx)

    bh = data.pct_change().mean(axis=1)
    bh_eq = (1 + bh).cumprod().loc[eq_idx[0]:]

    ar, av, sh, dd = metrics(eq)
    bh_ar, bh_av, bh_sh, bh_dd = metrics(bh_eq)

    print("\n" + "="*54)
    print("ARKHE CORE - DADOS REAIS (Yahoo Finance)")
    print("="*54)
    print(f"Periodo:          {dates[60].date()} a {dates[-1].date()}")
    print(f"Dias operados:    {len(trader.history)} | stop: {stopped}")
    print(f"\nARKHE CORE:")
    print(f"  Retorno Anual:  {ar*100:+.2f}%")
    print(f"  Volatilidade:   {av*100:.2f}%")
    print(f"  Sharpe Ratio:   {sh:.2f}")
    print(f"  Max Drawdown:   {dd*100:.2f}%")
    print(f"  Equity Final:   {eq.iloc[-1]:.4f}")
    print(f"\nBUY & HOLD (equal-weight):")
    print(f"  Retorno Anual:  {bh_ar*100:+.2f}%")
    print(f"  Volatilidade:   {bh_av*100:.2f}%")
    print(f"  Sharpe Ratio:   {bh_sh:.2f}")
    print(f"  Max Drawdown:   {bh_dd*100:.2f}%")
    print(f"  Equity Final:   {bh_eq.iloc[-1]:.4f}")
    print(f"\nAlpha vs B&H:     {(ar - bh_ar)*100:+.2f}% anual")
    print("="*54)
