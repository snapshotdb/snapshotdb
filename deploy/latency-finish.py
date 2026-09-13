#!/usr/bin/env python3
"""A stopped/timed-out service must not leave a latency report looking successful."""
import json
import os
from pathlib import Path
import sys

path = Path(sys.argv[1])
report = json.loads(path.read_text()) if path.exists() else {}
result = os.environ.get("SERVICE_RESULT", "unknown")
if result != "success" or report.get("state") != "passed":
    report.update(state="failed", passed=False)
    report.setdefault("error", "latency test service ended: "+result)
report["service_result"] = result
path.write_text(json.dumps(report, indent=2)+"\n")
