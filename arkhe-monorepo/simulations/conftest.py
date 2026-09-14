"""Torna o diretório `simulations` importável durante os testes pytest."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))