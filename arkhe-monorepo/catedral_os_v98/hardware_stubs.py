"""
hardware_stubs.py — Stubs honestos para hardware de RF (SDR e LoRa).
Todos os métodos que requerem hardware real levantam NotImplementedError.
"""
from typing import Optional


class SDRHardware:
    """Stub para SDR (HackRF, USRP, etc.)."""

    def __init__(self, device_type: str = "hackrf"):
        self.device_type = device_type
        self._connected = False

    def connect(self) -> bool:
        self._connected = True
        return True

    def start_sweep(self, start_mhz: float, end_mhz: float, bandwidth: float = 10.0):
        raise NotImplementedError(
            f"SDR sweep requer hardware {self.device_type} e pacote soapy_power instalado. "
            f"Simulação não disponível para operações de RF reais."
        )

    def transmit(self, frequency_mhz: float, data: bytes, gain: float = 0.0):
        raise NotImplementedError(
            f"Transmissão SDR requer hardware {self.device_type} configurado. "
            f"Nenhuma transmissão real pode ser realizada neste stub."
        )


class LoRaHardware:
    """Stub para LoRa (RFM95/RFM96)."""

    def __init__(self, spi_channel: int = 0, cs_pin: int = 1, frequency: float = 915.0):
        self.spi_channel = spi_channel
        self.cs_pin = cs_pin
        self.frequency = frequency
        self._initialized = False

    def initialize(self) -> bool:
        self._initialized = True
        return True

    def send(self, message: bytes, retries: int = 2, timeout: float = 1.0):
        raise NotImplementedError(
            f"Transmissão LoRa requer hardware RFM95/RFM96 conectado via SPI (CS={self.cs_pin}). "
            f"Instale pyLoraRFM9x ou adafruit_rfm9x e conecte o módulo para operação real."
        )

    def receive(self, timeout: float = 1.0) -> Optional[bytes]:
        raise NotImplementedError(
            f"Recepção LoRa requer hardware RFM95/RFM96 configurado. "
            f"Nenhuma recepção real disponível no stub."
        )
