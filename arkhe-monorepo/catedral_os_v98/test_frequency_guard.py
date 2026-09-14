import unittest
from decimal import Decimal
from frequency_guard import FrequencyGuard, Classification


class TestFrequencyGuard(unittest.TestCase):
    def setUp(self):
        self.guard = FrequencyGuard()

    def test_emergency(self):
        self.assertFalse(self.guard.is_allowed(121.5))
        self.assertEqual(self.guard.get_reason(121.5), "emergency_121_5")

    def test_aviation(self):
        for f in [108.0, 118.0, 137.0]:
            self.assertFalse(self.guard.is_allowed(f))
        self.assertTrue(self.guard.is_allowed(107.999))
        self.assertTrue(self.guard.is_allowed(137.001))

    def test_cospas_ul(self):
        self.assertFalse(self.guard.is_allowed(406.0))
        self.assertFalse(self.guard.is_allowed(406.1))
        self.assertTrue(self.guard.is_allowed(405.999))
        self.assertTrue(self.guard.is_allowed(406.101))

    def test_cospas_dl(self):
        self.assertFalse(self.guard.is_allowed(1544.0))
        self.assertFalse(self.guard.is_allowed(1545.0))
        # 1543.999 está em MSS DL (1525–1559) → deny
        self.assertFalse(self.guard.is_allowed(1543.999))
        self.assertEqual(self.guard.get_reason(1543.999), "mss_downlink_1525_1559")

    def test_gnss(self):
        self.assertFalse(self.guard.is_allowed(1559.0))
        self.assertFalse(self.guard.is_allowed(1610.0))
        self.assertFalse(self.guard.is_allowed(1575.42))
        # 1558.999 está dentro de MSS DL (1525–1559) → deny
        self.assertFalse(self.guard.is_allowed(1558.999))
        self.assertEqual(self.guard.get_reason(1558.999), "mss_downlink_1525_1559")
        # 1524.999 está abaixo de MSS DL → allowed
        self.assertTrue(self.guard.is_allowed(1524.999))
        # 1610.001 está acima de GNSS → allowed
        self.assertTrue(self.guard.is_allowed(1610.001))

    def test_tv_30(self):
        self.assertFalse(self.guard.is_allowed(250.0))
        self.assertFalse(self.guard.is_allowed(322.0))
        self.assertTrue(self.guard.is_allowed(249.999))
        self.assertTrue(self.guard.is_allowed(322.001))

    def test_5g_bands(self):
        self.assertFalse(self.guard.is_allowed(2350.0))   # 2.3 GHz
        self.assertFalse(self.guard.is_allowed(3500.0))   # 3.5 GHz
        self.assertTrue(self.guard.is_allowed(2299.999))
        self.assertTrue(self.guard.is_allowed(3800.001))

    def test_allowed_frequencies(self):
        # TV VHF, 70 cm, ISM não estão na deny-list
        for f in [55.0, 425.0, 433.0, 915.0, 2442.0]:
            self.assertTrue(self.guard.is_allowed(f))
        self.assertEqual(self.guard.get_reason(55.0), "allowed")
        # 2400.0 é o limite superior da faixa 5G 2300–2400 → deny
        self.assertFalse(self.guard.is_allowed(2400.0))

    def test_invalid_input(self):
        self.assertFalse(self.guard.is_allowed("nope"))
        self.assertFalse(self.guard.is_allowed(float('nan')))
        self.assertFalse(self.guard.is_allowed(float('inf')))
        self.assertEqual(self.guard.get_reason("nope"), "invalid_input")

    def test_csv_roundtrip(self):
        import tempfile
        with tempfile.NamedTemporaryFile(mode='w', suffix='.csv', delete=False) as f:
            self.guard.to_csv(f.name)
        loaded = FrequencyGuard.from_csv(f.name)
        self.assertEqual(len(loaded.bands), len(self.guard.bands))
        for (l1, h1, r1), (l2, h2, r2) in zip(self.guard.bands, loaded.bands):
            self.assertEqual(l1, l2)
            self.assertEqual(h1, h2)
            self.assertEqual(r1, r2)


if __name__ == "__main__":
    unittest.main()
