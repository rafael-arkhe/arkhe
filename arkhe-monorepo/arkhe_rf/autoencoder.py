"""
Autoencoder multi-modal RF + ELF — detecção de anomalias temporais.

A entrada é o concatenação restrita das duas modalidades:
  - RF  : vetor de features 68-dim (spectral, doppler).
  - ELF : vetor de features 20-dim (Schumann: amplitudes/fases/harmónicos).

O modelo aprende a representação latente de "normalidade"; qualquer bloco
cujo erro de reconstrução exceda μ + k·σ é rotulado como anomalia. O
resultado é devolvido como evento pronto para registro em ARKHE.

Requer PyTorch (`pip install "arkhe-rf[ml]"`). Sem torch, o módulo
continua importável e as chamadas de treino relatam o requisito.
"""

import numpy as np

from . import config as _config

__all__ = [
    "TORCH_AVAILABLE",
    "MultiModalAutoencoder",
    "ReconstructionAnomalyDetector",
    "train_autoencoder",
    "anomaly_event",
]

try:
    import torch
    import torch.nn as nn

    TORCH_AVAILABLE = True
except ImportError:  # pragma: no cover
    torch = None
    nn = None
    TORCH_AVAILABLE = False


if TORCH_AVAILABLE:  # pragma: no cover

    class MultiModalAutoencoder(nn.Module):
        """
        Encoder duplo (RF, ELF) → fusão latente → decoder simétrico.

        Dimensões padrão vêm de `config`; os hiddens seguem a arquitetura
        definida na Fase 2 (RF 68→64→32, ELF 20→32→16, latente 32).
        """

        def __init__(
            self,
            rf_dim: int = _config.RF_FEATURE_DIM,
            elf_dim: int = _config.ELF_FEATURE_DIM,
            latent_dim: int = _config.AE_LATENT_DIM,
            hidden_rf: int = _config.AE_HIDDEN_RF,
            hidden_elf: int = _config.AE_HIDDEN_ELF,
            decoder_hidden: int = _config.AE_DECODER_HIDDEN,
        ):
            super().__init__()
            self.rf_dim = rf_dim
            self.elf_dim = elf_dim
            self.latent_dim = latent_dim

            # Encoder RF
            self.rf_enc = nn.Sequential(
                nn.Linear(rf_dim, hidden_rf),
                nn.ReLU(),
                nn.Linear(hidden_rf, 32),
            )
            # Encoder ELF
            self.elf_enc = nn.Sequential(
                nn.Linear(elf_dim, hidden_elf),
                nn.ReLU(),
                nn.Linear(hidden_elf, 16),
            )
            # Fusão latente 32+16 → latent_dim
            self.fusion = nn.Linear(32 + 16, latent_dim)
            # Decoder simétrico
            self.decoder = nn.Sequential(
                nn.Linear(latent_dim, decoder_hidden),
                nn.ReLU(),
                nn.Linear(decoder_hidden, rf_dim + elf_dim),
            )

        def encode(self, rf: "torch.Tensor", elf: "torch.Tensor") -> "torch.Tensor":
            z_rf = self.rf_enc(rf)
            z_elf = self.elf_enc(elf)
            return self.fusion(torch.cat([z_rf, z_elf], dim=1))

        def forward(
            self, rf: "torch.Tensor", elf: "torch.Tensor"
        ) -> tuple["torch.Tensor", "torch.Tensor"]:
            z = self.encode(rf, elf)
            out = self.decoder(z)
            rf_out = out[:, : self.rf_dim]
            elf_out = out[:, self.rf_dim :]
            return rf_out, elf_out

        def reconstruction_loss(
            self, rf: "torch.Tensor", elf: "torch.Tensor", weight: float = 1.0
        ) -> "torch.Tensor":
            rf_recon, elf_recon = self.forward(rf, elf)
            mse = nn.MSELoss(reduction="mean")
            return mse(rf_recon, rf) + weight * mse(elf_recon, elf)

        def reconstruction_error(
            self, rf: "torch.Tensor", elf: "torch.Tensor"
        ) -> tuple["torch.Tensor", "torch.Tensor", "torch.Tensor"]:
            """Erro por amostra (vetor B×1 cada modalidade) e total."""
            rf_recon, elf_recon = self.forward(rf, elf)
            err_rf = ((rf_recon - rf) ** 2).mean(dim=1)
            err_elf = ((elf_recon - elf) ** 2).mean(dim=1)
            return err_rf, err_elf, err_rf + _config.AE_ELF_LOSS_WEIGHT * err_elf

    class ReconstructionAnomalyDetector:
        """
        Treina `MultiModalAutoencoder` sobre operação normal e detecta
        desvios com limiar μ + k·σ do erro de reconstrução (espaço padrão).
        """

        def __init__(
            self,
            rf_dim: int = _config.RF_FEATURE_DIM,
            elf_dim: int = _config.ELF_FEATURE_DIM,
            latent_dim: int = _config.AE_LATENT_DIM,
            sigma: float = _config.AE_SIGMA_ANOMALY,
            seed: int | None = 0,
            device: str = "cpu",
        ):
            if not TORCH_AVAILABLE:
                raise ImportError(
                    "PyTorch é necessário. `pip install 'arkhe-rf[ml]'`"
                )
            torch.manual_seed(seed)
            np.random.seed(seed)
            self.device = device
            self.sigma = float(sigma)
            self.model = MultiModalAutoencoder(
                rf_dim=rf_dim, elf_dim=elf_dim, latent_dim=latent_dim
            ).to(device)
            self.rf_dim, self.elf_dim = rf_dim, elf_dim
            self.rf_mean = np.zeros(rf_dim, dtype=np.float64)
            self.rf_std = np.ones(rf_dim, dtype=np.float64)
            self.elf_mean = np.zeros(elf_dim, dtype=np.float64)
            self.elf_std = np.ones(elf_dim, dtype=np.float64)
            self.err_mean_: float = 0.0
            self.err_std_: float = 1.0
            self.fitted_: bool = False

        # ------------------------------------------------------------
        def _standardize(self, rf: np.ndarray, elf: np.ndarray):
            rf_z = (rf - self.rf_mean) / (self.rf_std + 1e-8)
            elf_z = (elf - self.elf_mean) / (self.elf_std + 1e-8)
            t_rf = torch.from_numpy(rf_z.astype(np.float32)).to(self.device)
            t_elf = torch.from_numpy(elf_z.astype(np.float32)).to(self.device)
            return t_rf, t_elf

        def fit(
            self,
            rf: np.ndarray,
            elf: np.ndarray,
            epochs: int = _config.AE_EPOCHS,
            batch_size: int = _config.AE_BATCH_SIZE,
            lr: float = _config.AE_LR,
            verbose: int = 0,
            val_fraction: float = 0.2,
        ) -> dict:
            """
            Ajusta scalers e treina o autoencoder sobre dados normais,
            calibrando μ,σ do erro num subconjunto de validação (evita
            limiar anémico por memorização do treino).

            Retorna histórico de perdas por época de treino.
            """
            rf = np.asarray(rf, dtype=np.float64)
            elf = np.asarray(elf, dtype=np.float64)
            if rf.ndim != 2 or elf.ndim != 2:
                raise ValueError("rf/elf devem ser (n_amostras, dim)")
            if len(rf) < 8:
                raise ValueError("poucos exemplos para treino+validação")

            perm = np.random.default_rng(0).permutation(len(rf))
            n_tr = max(4, int(len(rf) * (1.0 - val_fraction)))
            tr_idx = perm[:n_tr]
            va_idx = perm[n_tr:]

            self.rf_mean = rf[tr_idx].mean(axis=0)
            self.rf_std = rf[tr_idx].std(axis=0) + 1e-12
            self.elf_mean = elf[tr_idx].mean(axis=0)
            self.elf_std = elf[tr_idx].std(axis=0) + 1e-12

            t_tr = self._standardize(rf[tr_idx], elf[tr_idx])
            history = train_autoencoder(
                self.model,
                t_tr[0].cpu().numpy(),
                t_tr[1].cpu().numpy(),
                epochs=epochs,
                batch_size=batch_size,
                lr=lr,
                verbose=verbose,
            )
            # Distribuição do erro em validação (operacão normal real de teste).
            t_va = self._standardize(rf[va_idx], elf[va_idx])
            _, _, tot = self._model_errors(*t_va)
            errs = tot.cpu().numpy()
            self.err_mean_ = float(errs.mean())
            self.err_std_ = float(errs.std() + 1e-12)
            self.fitted_ = True
            return history

        def _model_errors(self, t_rf, t_elf):
            with torch.no_grad():
                return self.model.reconstruction_error(t_rf, t_elf)

        def detect(self, rf: np.ndarray, elf: np.ndarray) -> dict:
            """
            Avalia blocos (per-sample). Anomalia se total > μ + kσ.
            """
            if not self.fitted_:
                raise RuntimeError("chame .fit() antes de detect()")
            rf = np.atleast_2d(np.asarray(rf, dtype=np.float64))
            elf = np.atleast_2d(np.asarray(elf, dtype=np.float64))
            t_rf, t_elf = self._standardize(rf, elf)
            err_rf, err_elf, tot = self._model_errors(t_rf, t_elf)
            threshold = self.err_mean_ + self.sigma * self.err_std_
            flags = tot.cpu().numpy() > threshold
            return {
                "err_rf": err_rf.cpu().numpy(),
                "err_elf": err_elf.cpu().numpy(),
                "err_total": tot.cpu().numpy(),
                "threshold": float(threshold),
                "is_anomaly": flags,
            }

        def threshold(self) -> float:
            return self.err_mean_ + self.sigma * self.err_std_

        def to_arkhe_event(
            self,
            rf: np.ndarray,
            elf: np.ndarray,
            timestamps: np.ndarray | None = None,
        ) -> list[dict]:
            """Converte resultados de detecção em eventos ARKHE (T1-VIBRA-2)."""
            res = self.detect(rf, elf)
            events = []
            for i, flag in enumerate(res["is_anomaly"]):
                events.append(
                    anomaly_event(
                        total_err=float(res["err_total"][i]),
                        threshold=res["threshold"],
                        sigma=self.sigma,
                        ts=float(timestamps[i]) if timestamps is not None else None,
                        anomaly=bool(flag),
                    )
                )
            return events

    def train_autoencoder(
        model: MultiModalAutoencoder,
        rf: np.ndarray,
        elf: np.ndarray,
        epochs: int = _config.AE_EPOCHS,
        batch_size: int = _config.AE_BATCH_SIZE,
        lr: float = _config.AE_LR,
        verbose: int = 0,
    ) -> dict:
        """
        Treina o modelo com SGD/Adam em minilotes. `rf`/`elf` devem estar
        padronizados (z-score). Retorna histórico {epoch: loss}.
        """
        rf = np.asarray(rf, dtype=np.float32)
        elf = np.asarray(elf, dtype=np.float32)
        n = len(rf)
        device = next(model.parameters()).device
        optimizer = torch.optim.Adam(model.parameters(), lr=lr)
        history: dict[int, float] = {}
        n_batches = max(1, int(np.ceil(n / batch_size)))
        for epoch in range(1, int(epochs) + 1):
            perm = np.random.permutation(n)
            losses = []
            for b in range(n_batches):
                idx = perm[b * batch_size : (b + 1) * batch_size]
                if idx.size == 0:
                    continue
                t_rf = torch.from_numpy(rf[idx]).to(device)
                t_elf = torch.from_numpy(elf[idx]).to(device)
                optimizer.zero_grad()
                loss = model.reconstruction_loss(t_rf, t_elf)
                loss.backward()
                optimizer.step()
                losses.append(float(loss.detach().cpu().item()))
            history[epoch] = float(np.mean(losses))
            if verbose and epoch % (epochs // 10 or 1) == 0:
                print(f"epoch {epoch}/{epochs} loss {history[epoch]:.6f}")
        return history


else:  # torch ausente — API viva, execução condicional.

    def _no_torch(*args, **kwargs):
        raise ImportError("PyTorch é necessário. `pip install 'arkhe-rf[ml]'`")

    MultiModalAutoencoder = _no_torch  # type: ignore[assignment]
    ReconstructionAnomalyDetector = _no_torch  # type: ignore[assignment]

    def train_autoencoder(*args, **kwargs):  # type: ignore[misc]
        return _no_torch(*args, **kwargs)


def anomaly_event(
    total_err: float,
    threshold: float,
    sigma: float,
    ts: float | None = None,
    anomaly: bool = True,
    channel: tuple[str, str] = ("RF", "ELF"),
) -> dict:
    """
    Monta o registro ARKHE de uma anomalia temporal (formato estável).

    A correlação com HNDL/λ₂ e o disparo T1-VIBRA-2 são feitos a montante
    (LivingGeometryDetector); este evento é o "sensor" bruto dessa cadeia.
    """
    return {
        "kind": "TEMPORAL_ANOMALY",
        "sigma": sigma,
        "reconstruction_error": float(total_err),
        "threshold": float(threshold),
        "exceeds_by_sigma": float((total_err - threshold) / (threshold + 1e-12)),
        "is_anomaly": bool(anomaly),
        "channels": list(channel),
        "unix_epoch_s": round(float(ts), 6) if ts is not None else None,
    }