# catedral_os_v152/tests/conftest.py
import os
import sys

_PY = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "python")
sys.path.insert(0, os.path.abspath(_PY))