"""Garante que o pacote `arkhe_rf` seja importável de qualquer raiz."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))