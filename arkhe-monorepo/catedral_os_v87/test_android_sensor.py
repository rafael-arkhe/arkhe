#!/usr/bin/env python3
import unittest
from unittest.mock import patch, MagicMock

import android_sensor as s


class TestPlmnMap(unittest.TestCase):
    def test_br_codes(self):
        self.assertEqual(s.operator_from_plmn("72405"), "Claro")
        self.assertEqual(s.operator_from_plmn("72410"), "Vivo")
        self.assertEqual(s.operator_from_plmn("72402"), "TIM")
        self.assertEqual(s.operator_from_plmn("72431"), "Oi")
        self.assertEqual(s.operator_from_plmn("72415"), "Algar")
        self.assertEqual(s.operator_from_plmn("99999"), "99999")

    def test_operator_fallback(self):
        # Quando PLMN não está na tabela, retorna o próprio PLMN
        self.assertEqual(s.operator_from_plmn("72499"), "72499")


class TestParse(unittest.TestCase):
    def test_claro_not_labeled_vivo(self):
        dump = """
        mHomePlmn=72405
        mOperatorNumeric=72405
        mOperatorAlphaShort=VIVO
        mNetworkType=13
        mRoaming=false
        """
        info = s.parse_telephony_dump(dump)
        self.assertEqual(info.plmn, "72405")
        self.assertEqual(info.operator, "Claro")  # table wins over alpha
        self.assertEqual(info.network_type, "LTE")
        self.assertIs(info.is_roaming, False)
        self.assertNotIn("roaming", info.context_fragment())

    def test_connected_is_not_roaming(self):
        dump = "mServiceState=0\nmDataConnectionState=2\n"
        info = s.parse_telephony_dump(dump)
        self.assertIsNone(info.is_roaming)

    def test_dual_sim_phone_id(self):
        dump = """
        Phone Id=1
        mHomePlmn=72410
        mOperatorNumeric=72410
        """
        info = s.parse_telephony_dump(dump)
        self.assertEqual(info.phone_id, 1)
        self.assertEqual(info.plmn, "72410")
        self.assertEqual(info.operator, "Vivo")

    def test_voice_data_operator_fallback(self):
        dump = """
        mVoiceOperatorNumeric=72405
        mDataOperatorNumeric=72410
        """
        info = s.parse_telephony_dump(dump)
        # mOperatorNumeric não está presente, usa voice como fallback
        self.assertEqual(info.plmn_network, "72405")
        self.assertEqual(info.operator, "Claro")


class TestGetPlmn(unittest.TestCase):
    @patch("android_sensor.shutil.which", return_value=None)
    def test_no_adb(self, _):
        with self.assertRaises(s.AdbMissing):
            s.get_plmn()

    @patch("android_sensor.shutil.which", return_value="/usr/bin/adb")
    @patch("subprocess.run")
    def test_get_plmn_success(self, mock_run, _):
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = "72405\n"
        mock_run.return_value = mock_result

        result = s.get_plmn()
        self.assertEqual(result, "72405")

    @patch("android_sensor.shutil.which", return_value="/usr/bin/adb")
    @patch("subprocess.run")
    def test_get_plmn_failure(self, mock_run, _):
        mock_result = MagicMock()
        mock_result.returncode = 1
        mock_result.stdout = ""
        mock_run.return_value = mock_result

        result = s.get_plmn()
        self.assertIsNone(result)


class TestListDevices(unittest.TestCase):
    @patch("android_sensor.shutil.which", return_value="/usr/bin/adb")
    @patch("subprocess.run")
    def test_list_devices(self, mock_run, _):
        mock_result = MagicMock()
        mock_result.stdout = "List of devices attached\n123456\tdevice\n789012\tdevice\n"
        mock_run.return_value = mock_result

        devices = s.list_devices()
        self.assertEqual(devices, ["123456", "789012"])


if __name__ == "__main__":
    unittest.main()
