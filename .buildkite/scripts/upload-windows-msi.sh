#!/usr/bin/env bash
# Dynamically upload the Windows MSI build step when WINDOWS_AGENT_QUEUE is set.
# Soft-skips (exit 0) when the queue env var is missing so the parent pipeline
# still succeeds before agents / queues are provisioned.
set -euo pipefail

QUEUE="${WINDOWS_AGENT_QUEUE:-}"

if [[ -z "${QUEUE}" ]]; then
  cat <<'EOF' | buildkite-agent annotate --style "warning" --context "windows-msi"
### Windows MSI skipped

`WINDOWS_AGENT_QUEUE` is not set on this pipeline.

1. Create a Windows queue in Buildkite (Agents → Queues)
2. Set pipeline env `WINDOWS_AGENT_QUEUE=<queue-key>`
3. Re-run the build

See README § “Buildkite 自动打 MSI”.
EOF
  echo "WINDOWS_AGENT_QUEUE unset — skipping MSI upload" >&2
  exit 0
fi

cat <<EOF | buildkite-agent pipeline upload
steps:
  - label: ":package: Windows MSI (MSVC + WiX)"
    key: windows-msi
    command: powershell -NoProfile -ExecutionPolicy Bypass -File .buildkite/scripts/windows-msi.ps1
    agents:
      queue: "${QUEUE}"
    artifact_paths:
      - "target/wix/*.msi"
    timeout_in_minutes: 90
EOF
