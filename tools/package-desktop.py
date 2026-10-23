"""Package the native desktop release using the shared provenance/license collector."""
from pathlib import Path
import runpy
import sys

if __name__ == '__main__':
    sys.argv.insert(1, '--desktop')
    runpy.run_path(str(Path(__file__).with_name('package-demo.py')), run_name='__main__')
