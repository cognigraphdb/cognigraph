#!/bin/bash

set -euo pipefail

hook_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec python3 "$hook_dir/protect-files.py"
