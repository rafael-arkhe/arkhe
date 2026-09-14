#!/usr/bin/env python3
"""
test_anatel_band_guard.py — Testes para o módulo de guarda de bandas
"""
import unittest
from anatel_band_guard import (
    is_frequency_allowed,
    get_restriction_reason,
    RESTRICTED_BANDS,
)

class TestAnatelBandGuard(unittest.TestCase):
    def test_aviation_bands(self):
        """Testa faixas da aviação"""
        # Frequências críticas
        self.assertFalse(is_frequency_allowed(108.0))
        self.assertFalse(is_frequency_allowed(118.0))
        self.assertFalse(is_frequency_allowed(121.5))
        self.assertFalse(is_frequency_allowed(137.0))

    def test_emergency_freq(self):
        """Testa a frequência de emergência"""
        self.assertFalse(is_frequency_allowed(121.5))
        self.assertEqual(get_restriction_reason(121.5), "Frequência de emergência (121.5 MHz)")

    def test_satellite_band(self):
        self.assertFalse(is_frequency_allowed(406.05))

    def test_broadcast_bands(self):
        self.assertFalse(is_frequency_allowed(0.6))   # AM
        self.assertFalse(is_frequency_allowed(100.0)) # FM

    def test_ham_bands(self):
        self.assertFalse(is_frequency_allowed(145.0)) # VHF ham (secundário)
        self.assertFalse(is_frequency_allowed(435.0)) # UHF ham

    def test_allowed_frequencies(self):
        """Testa frequências permitidas"""
        # ISM bands
        self.assertTrue(is_frequency_allowed(433.0))   # ISM UHF
        self.assertTrue(is_frequency_allowed(868.0))   # ISM EU
        self.assertTrue(is_frequency_allowed(915.0))   # ISM US
        self.assertTrue(is_frequency_allowed(2400.0))  # ISM 2.4 GHz

    def test_reason_message(self):
        """Testa mensagens de restrição"""
        self.assertIn("Aviação", get_restriction_reason(120.0))
        self.assertIn("Radioamador", get_restriction_reason(145.0))

    def test_band_list_consistency(self):
        """Verifica se as faixas estão ordenadas (low <= high; pontos são permitidos)"""
        for low, high in RESTRICTED_BANDS:
            self.assertLessEqual(low, high, f"Faixa inválida: {low} > {high}")

if __name__ == "__main__":
    unittest.main()
