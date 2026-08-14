import importlib.util
import numpy as np
import pandas as pd
import yfinance as yf
from datetime import datetime, timedelta

spec = importlib.util.spec_from_file_location("v52", "arkhe_v52.py")
A = importlib.util.module_from_spec(spec)
spec.loader.exec_module(A)

syms = ['AAPL', 'MSFT', 'GOOG', 'AMZN', 'TSLA']
end = datetime.now(); start = end - timedelta(days=3*365)
print("Baixando dados reais...")
data = yf.download(syms, start=start, end=end, interval='1d', auto_adjust=False, progress=False)['Adj Close'].dropna()
dates = data.index
print(f"Dias: {len(dates)} | {dates[0].date()} a {dates[-1].date()}")

trader = A.ArkheTrader(syms)
for i in range(60, len(dates) - 1):
    pd_ = {s: data[s].iloc[:i+1].values for s in syms}
    rd_ = {s: (data[s].iloc[i+1] - data[s].iloc[i]) / data[s].iloc[i] for s in syms}
    if trader.run_day(pd_, rd_):
        print(f"KALI_YUGA em {dates[i].date()}"); break

eq_s = pd.Series(trader.hist, index=dates[60:len(trader.hist)+60])
r_s = eq_s.pct_change().dropna()
ar, av = r_s.mean()*252, r_s.std()*np.sqrt(252)
sh = ar/av if av > 0 else 0
dd = (eq_s / eq_s.expanding().max() - 1).min()

bh = data.pct_change().mean(axis=1)
bh_eq = (1 + bh).cumprod()
bh_ar, bh_av = bh.mean()*252, bh.std()*np.sqrt(252)
bh_sh = bh_ar/bh_av if bh_av > 0 else 0
bh_dd = (bh_eq / bh_eq.expanding().max() - 1).min()

print("\n" + "="*54)
print("ARKHE TRADER v5.2 - DADOS REAIS (Yahoo Finance)")
print("="*54)
print(f"Período:          {dates[60].date()} a {dates[-1].date()}")
print(f"Dias operados:    {len(trader.hist)}")
print(f"\nARKHE:")
print(f"  Retorno Anual:  {ar*100:+.2f}%")
print(f"  Volatilidade:   {av*100:.2f}%")
print(f"  Sharpe Ratio:   {sh:.2f}")
print(f"  Max Drawdown:   {dd*100:.2f}%")
print(f"  Equity Final:   {eq_s.iloc[-1]:.4f}")
print(f"  Zeros/Polos:    {len(trader.geom.zeros)}/{len(trader.geom.poles)}")
print(f"\nBUY & HOLD (equal-weight):")
print(f"  Retorno Anual:  {bh_ar*100:+.2f}%")
print(f"  Volatilidade:   {bh_av*100:.2f}%")
print(f"  Sharpe Ratio:   {bh_sh:.2f}")
print(f"  Max Drawdown:   {bh_dd*100:.2f}%")
print(f"  Equity Final:   {bh_eq.iloc[-1]:.4f}")
print(f"\nAlpha vs B&H:     {(ar - bh_ar)*100:+.2f}% anual")
print("="*54)
