import sys
import os
import pytest

# Add src directory to path so relative imports work in tests
sys.path.insert(0, os.path.abspath('src'))
sys.exit(pytest.main(['tests/']))
