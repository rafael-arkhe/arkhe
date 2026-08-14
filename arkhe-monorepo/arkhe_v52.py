import numpy as np
import pandas as pd
from scipy.stats import ttest_1samp
from datetime import datetime, timedelta
import warnings
warnings.filterwarnings('ignore')

# ==============================================================
# GEOMETRIA HIPERBÓLICA
# ==============================================================
class PoincareDisk:
    @staticmethod
    def mobius_add(x, y):
        x2, y2, xy = np.dot(x,x), np.dot(y,y), np.dot(x,y)
        return ((1 + 2*xy + y2)*x + (1 - x2)*y) / (1 + 2*xy + x2*y2 + 1e-12)

    @staticmethod
    def exponential_map(x, v):
        v_norm = np.linalg.norm(v)
        if v_norm < 1e-8: return x
        lam = 2 / (1 - np.dot(x,x))
        move = np.tanh((lam * v_norm) / 2) * (v / v_norm)
        return PoincareDisk.mobius_add(x, move)

def poincare_c(z1, z2):
    num = abs(z1 - z2)**2
    den = (1 - abs(z1)**2) * (1 - abs(z2)**2)
    return np.arccosh(np.clip(1 + 2*num/(den+1e-12), 1.0, 1e12))

def hyp_grad(pot, z, eps=1e-6):
    dx = (pot(z + eps) - pot(z - eps)) / (2*eps)
    dy = (pot(z + eps*1j) - pot(z - eps*1j)) / (2*eps)
    return complex(dx, dy) * ((1 - abs(z)**2)**2) / 4

# ==============================================================
# MAPEAMENTO
# ==============================================================
class MarketMapper:
    def map(self, prices, vols):
        r = np.diff(np.log(prices))[-20:]
        if len(r) < 5: return 0j
        v = np.std(r)
        rk = np.searchsorted(np.sort(vols), v) / len(vols)
        rad = np.clip(0.1 + 0.8 * rk, 0.01, 0.98)
        sl, _ = np.polyfit(np.arange(20), prices[-20:], 1)
        th = np.arctan(np.clip(((sl/np.mean(prices[-20:]))/(v+1e-8)), -5, 5)) % (2*np.pi)
        return complex(rad * np.cos(th), rad * np.sin(th))

# ==============================================================
# GEOMETRIA ADAPTATIVA (inerte na prática, mantida para compatibilidade)
# ==============================================================
class AdaptiveGeometry:
    def __init__(self):
        self.zeros, self.poles, self.t = [], [], 0
    def update(self, z, pnl):
        self.t += 1
        self.zeros = [(p, w*np.exp(-0.023*(self.t-ts)), ts) for p,w,ts in self.zeros if w*np.exp(-0.023*(self.t-ts))>0.01]
        self.poles = [(p, w*np.exp(-0.023*(self.t-ts)), ts) for p,w,ts in self.poles if w*np.exp(-0.023*(self.t-ts))>0.01]
        if len(pnl) >= 15:
            st, pv = ttest_1samp(pnl, 0)
            if st > 1.0 and not any(poincare_c(z, p)<0.15 for p,_,_ in self.zeros):
                self.zeros.append((z, min(abs(np.mean(pnl))*10, 0.5), self.t))
            elif st < -1.0 and not any(poincare_c(z, p)<0.15 for p,_,_ in self.poles):
                self.poles.append((z, min(abs(np.mean(pnl))*10, 0.5), self.t))
        self.zeros = sorted(self.zeros, key=lambda x: x[1], reverse=True)[:15]
        self.poles = sorted(self.poles, key=lambda x: x[1], reverse=True)[:15]
    def potential(self, z):
        v = sum(-w*np.log(poincare_c(z,p)+0.01) for p,w,_ in self.zeros)
        v += sum(w*np.log(poincare_c(z,p)+0.01) for p,w,_ in self.poles)
        return v

# ==============================================================
# MOTOR (DEADLOCK CORRIGIDO — sinal base sempre ativo)
# ==============================================================
class ArkheTrader:
    def __init__(self, syms):
        self.syms = syms
        self.mapper, self.geom = MarketMapper(), AdaptiveGeometry()
        self.pos = {s: 0.0 for s in syms}
        self.state = {s: 0j for s in syms}
        self.eq, self.hist, self.pnl_h = 1.0, [], {s: [] for s in syms}

    def run_day(self, prices_dict, rets_dict):
        vols = [np.std(np.diff(np.log(p))[-20:]) for p in prices_dict.values() if len(p)>20]
        for s, p in prices_dict.items():
            if len(p) >= 20: self.state[s] = self.mapper.map(p, vols)

        new_pos = {}
        for s, z in self.state.items():
            # SINAL BASE: momentum via ângulo θ (sempre ativo)
            theta = np.angle(z)
            base_signal = np.cos(theta) * abs(z)

            # MODULAÇÃO HIPERBÓLICA (quando houver zeros/polos)
            if self.geom.zeros or self.geom.poles:
                z_cur = z
                for _ in range(3):
                    g = hyp_grad(self.geom.potential, z_cur)
                    if abs(g) < 1e-12: break
                    z_cur = complex(*PoincareDisk.exponential_map(
                        np.array([z_cur.real, z_cur.imag]),
                        np.array([g.real, g.imag])*0.02))
                pot_diff = self.geom.potential(z_cur) - self.geom.potential(z)
                modulation = 1.0 + np.clip(pot_diff * 2.0, -0.5, 0.5)
                base_signal *= modulation

            new_pos[s] = np.clip(base_signal * 0.3, -0.3, 0.3)

        tot = sum(abs(new_pos[s]) for s in self.syms)
        if tot > 0.8: new_pos = {s: w*(0.8/tot) for s, w in new_pos.items()}

        cost = sum(abs(new_pos[s] - self.pos[s]) for s in self.syms) * 0.0015
        gross = sum(self.pos[s] * rets_dict[s] for s in self.syms)
        self.eq *= (1 + gross - cost)

        for s in self.syms:
            if s in rets_dict:
                self.pnl_h[s].append(self.pos[s] * rets_dict[s])
                if len(self.pnl_h[s]) > 60: self.pnl_h[s].pop(0)

        if len(self.hist) >= 30 and len(self.hist) % 5 == 0:
            for s in self.syms:
                if len(self.pnl_h[s]) >= 15: self.geom.update(self.state[s], self.pnl_h[s])

        self.pos = new_pos
        self.hist.append(self.eq)
        return (max(self.hist) - self.eq) / max(self.hist) > 0.12

# ==============================================================
# EXECUÇÃO
# ==============================================================
if __name__ == "__main__":
    # Gerar dados sintéticos (substituir por yfinance para dados reais)
    np.random.seed(42)
    nd, mu, sg = 756, np.array([.12,.10,.08,.09,.15]), np.array([.28,.25,.30,.32,.45])
    cr = np.array([[1,.75,.7,.65,.55],[.75,1,.72,.68,.5],[.7,.72,1,.6,.58],[.65,.68,.6,1,.52],[.55,.5,.58,.52,1]])
    pr = np.vstack([np.ones(5), np.exp(np.cumsum((mu-.5*sg**2)*(1/252) + sg*np.sqrt(1/252)*np.random.normal(size=(nd,5))@np.linalg.cholesky(cr).T, axis=0))])*100
    dt = pd.date_range(end=datetime.now(), periods=nd+1, freq='B')
    data = pd.DataFrame(pr, index=dt, columns=['AAPL','MSFT','GOOG','AMZN','TSLA'])

    syms = list(data.columns)
    trader = ArkheTrader(syms)
    for i in range(60, len(dt) - 1):
        pd_ = {s: data[s].iloc[:i+1].values for s in syms}
        rd_ = {s: (data[s].iloc[i+1] - data[s].iloc[i]) / data[s].iloc[i] for s in syms}
        if trader.run_day(pd_, rd_):
            print(f"KALI_YUGA em {dt[i].date()}"); break

    eq_s = pd.Series(trader.hist, index=dt[60:len(trader.hist)+60])
    r_s = eq_s.pct_change().dropna()
    ar, av = r_s.mean()*252, r_s.std()*np.sqrt(252)
    sh = ar/av if av > 0 else 0
    dd = (eq_s / eq_s.expanding().max() - 1).min()
    print(f"Ret: {ar*100:+.2f}% | Vol: {av*100:.2f}% | Sharpe: {sh:.2f} | DD: {dd*100:.2f}% | Eq: {eq_s.iloc[-1]:.4f} | Zeros/Polos: {len(trader.geom.zeros)}/{len(trader.geom.poles)} | dias: {len(trader.hist)}")
