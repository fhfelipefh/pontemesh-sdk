#!/usr/bin/env bash
set -euo pipefail

SDK_REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVER_REPO="${PONTEMESH_SERVER_REPO:-$(cd "$SDK_REPO/../pontemesh-server" 2>/dev/null && pwd)}"
ARTIFACT_DIR="${PONTEMESH_FULL_STACK_ARTIFACT_DIR:-$SDK_REPO/target/full-stack-agent-gate}"
LOG_DIR="$ARTIFACT_DIR/logs"
REPORT_FILE="$ARTIFACT_DIR/report.md"
RUN_LOG="$LOG_DIR/run.log"
COMPOSE_OVERRIDE="$ARTIFACT_DIR/docker-compose.full-stack.override.yml"
COMPOSE_PROJECT="${PONTEMESH_FULL_STACK_COMPOSE_PROJECT:-pontemesh-sdk-live}"
WEB_HOST_PORT="${PONTEMESH_FULL_STACK_WEB_PORT:-18080}"
S3_HOST_PORT="${PONTEMESH_FULL_STACK_S3_PORT:-19000}"
CLIENT_COUNT="${PONTEMESH_FULL_STACK_CLIENTS:-30}"
CLIENT_PARALLELISM="${PONTEMESH_FULL_STACK_CLIENT_PARALLELISM:-8}"
OBJECT_BYTES="${PONTEMESH_FULL_STACK_OBJECT_BYTES:-262144}"
RESET="${PONTEMESH_FULL_STACK_RESET:-1}"
PULL="${PONTEMESH_FULL_STACK_PULL:-1}"
KEEP_STACK="${PONTEMESH_FULL_STACK_KEEP_STACK:-0}"
ADMIN_PASSWORD="${PONTEMESH_FULL_STACK_ADMIN_PASSWORD:-pm-admin-local-setup-agent}"
BUCKET="${PONTEMESH_FULL_STACK_BUCKET:-pontemesh-sdk-live}"
KEY="${PONTEMESH_FULL_STACK_KEY:-objects/full-stack-agent.bin}"
STAGE="bootstrap"
COMPOSE_STARTED=0
STARTED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

mkdir -p "$LOG_DIR"
: > "$RUN_LOG"
SERVER_REPO_DISPLAY="../$(basename "$SERVER_REPO")"
exec > >(sed -u -e "s|$SDK_REPO|.|g" -e "s|$SERVER_REPO|$SERVER_REPO_DISPLAY|g" | tee -a "$RUN_LOG") 2>&1

artifact_display_path() {
  local path="$1"
  case "$path" in
    "$SDK_REPO") printf '.\n' ;;
    "$SDK_REPO"/*) printf '%s\n' "${path#"$SDK_REPO"/}" ;;
    *)
      if command -v realpath >/dev/null 2>&1; then
        realpath --relative-to="$SDK_REPO" "$path" 2>/dev/null || basename "$path"
      else
        basename "$path"
      fi
      ;;
  esac
}

write_report() {
  local status="$1"
  local cleanup_status="$2"
  local finished_at
  local failed_stage="none"
  finished_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  if [[ "$status" != "passed" ]]; then
    failed_stage="$STAGE"
  fi
  {
    printf '# Ponte Mesh SDK Full Stack Gate\n\n'
    printf -- '- status: `%s`\n' "$status"
    printf -- '- startedAt: `%s`\n' "$STARTED_AT"
    printf -- '- finishedAt: `%s`\n' "$finished_at"
    printf -- '- failedStage: `%s`\n' "$failed_stage"
    printf -- '- cleanup: `%s`\n\n' "$cleanup_status"
    printf '## Configuration\n\n'
    printf -- '- serverRepo: `%s`\n' "$(artifact_display_path "$SERVER_REPO")"
    printf -- '- sdkRepo: `%s`\n' "$(artifact_display_path "$SDK_REPO")"
    printf -- '- composeProject: `%s`\n' "$COMPOSE_PROJECT"
    printf -- '- webHostPort: `%s`\n' "$WEB_HOST_PORT"
    printf -- '- s3HostPort: `%s`\n' "$S3_HOST_PORT"
    printf -- '- clientCount: `%s`\n' "$CLIENT_COUNT"
    printf -- '- clientParallelism: `%s`\n' "$CLIENT_PARALLELISM"
    printf -- '- objectBytes: `%s`\n' "$OBJECT_BYTES"
    printf -- '- reset: `%s`\n' "$RESET"
    printf -- '- pull: `%s`\n' "$PULL"
    printf -- '- keepStack: `%s`\n\n' "$KEEP_STACK"
    printf '## Repository Revisions\n\n'
    printf -- '- server: `%s`\n' "$(git -C "$SERVER_REPO" rev-parse HEAD 2>/dev/null || printf unavailable)"
    printf -- '- sdk: `%s`\n\n' "$(git -C "$SDK_REPO" rev-parse HEAD 2>/dev/null || printf unavailable)"
    printf '## Result\n\n'
    if [[ -f "$ARTIFACT_DIR/summary.json" ]]; then
      printf '```json\n'
      cat "$ARTIFACT_DIR/summary.json"
      printf '```\n\n'
    else
      printf 'No client summary was produced.\n\n'
    fi
    printf '## Artifacts\n\n'
    printf -- '- runLog: `%s`\n' "$(artifact_display_path "$RUN_LOG")"
    printf -- '- composeLog: `%s`\n' "$(artifact_display_path "$LOG_DIR/compose.log")"
    printf -- '- composePs: `%s`\n' "$(artifact_display_path "$LOG_DIR/compose-ps.txt")"
    printf -- '- setupAgent: `%s`\n' "$(artifact_display_path "$ARTIFACT_DIR/setup-agent.json")"
    printf -- '- objectMetadata: `%s`\n' "$(artifact_display_path "$ARTIFACT_DIR/object-meta.json")"
    printf -- '- applicationCredential: `%s`\n' "$(artifact_display_path "$ARTIFACT_DIR/create-application.json")"
    printf -- '- clientReports: `%s`\n' "$(artifact_display_path "$ARTIFACT_DIR/clients")"
    printf -- '- summary: `%s`\n' "$(artifact_display_path "$ARTIFACT_DIR/summary.json")"
  } > "$REPORT_FILE"
}

finish() {
  local exit_code="$?"
  local cleanup_status="not-started"
  if [[ "$COMPOSE_STARTED" == "1" ]]; then
    compose ps > "$LOG_DIR/compose-ps.txt" 2>&1 || true
    compose logs --no-color > "$LOG_DIR/compose.log" 2>&1 || true
    if [[ "$KEEP_STACK" == "1" ]]; then
      cleanup_status="kept"
    else
      compose down --volumes --remove-orphans > "$LOG_DIR/compose-down.log" 2>&1 || cleanup_status="failed"
      if [[ "$cleanup_status" != "failed" ]]; then
        cleanup_status="discarded"
      fi
    fi
  fi
  if (( exit_code == 0 )); then
    write_report "passed" "$cleanup_status"
  else
    write_report "failed" "$cleanup_status"
  fi
  printf '\nReport: %s\n' "$(artifact_display_path "$REPORT_FILE")"
  exit "$exit_code"
}

trap 'printf "\nFAILED at stage: %s\n" "$STAGE" >&2' ERR
trap finish EXIT

stage() {
  STAGE="$1"
  printf '\n==> %s\n' "$STAGE"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'Required command not found: %s\n' "$1" >&2
    exit 1
  fi
}

compose() {
  docker compose -p "$COMPOSE_PROJECT" -f "$SERVER_REPO/docker/docker-compose.yml" -f "$COMPOSE_OVERRIDE" "$@"
}

wait_for_http() {
  local url="$1"
  local attempts="${2:-90}"
  for _ in $(seq 1 "$attempts"); do
    if curl --silent --fail --output /dev/null "$url"; then
      return 0
    fi
    sleep 1
  done
  printf 'HTTP endpoint did not become ready: %s\n' "$url" >&2
  compose logs --no-color server >&2 || true
  exit 1
}

json_get() {
  python3 - "$1" "$2" <<'PY'
import json
import sys

path, selector = sys.argv[1], sys.argv[2].split(".")
with open(path, "r", encoding="utf-8") as handle:
    value = json.load(handle)
for part in selector:
    value = value[part]
print(value)
PY
}

mcp_call() {
  local tool="$1"
  local args_file="$2"
  local out_file="$3"
  python3 - "$tool" "$args_file" "$ARTIFACT_DIR/mcp-request.json" <<'PY'
import json
import sys

tool, args_path, out_path = sys.argv[1:4]
with open(args_path, "r", encoding="utf-8") as handle:
    arguments = json.load(handle)
payload = {
    "jsonrpc": "2.0",
    "id": tool,
    "method": "tools/call",
    "params": {"name": tool, "arguments": arguments},
}
with open(out_path, "w", encoding="utf-8") as handle:
    json.dump(payload, handle)
PY
  curl --silent --show-error --fail \
    -H "Authorization: Bearer $MCP_TOKEN" \
    -H "Content-Type: application/json" \
    --data-binary "@$ARTIFACT_DIR/mcp-request.json" \
    "$MCP_URL" > "$out_file"
  python3 - "$out_file" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    response = json.load(handle)
if response.get("error"):
    raise SystemExit(json.dumps(response["error"], indent=2))
result = response.get("result", {})
if "structuredContent" not in result:
    raise SystemExit("MCP response does not include structuredContent")
PY
}

structured_content() {
  python3 - "$1" "$2" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    response = json.load(handle)
with open(sys.argv[2], "w", encoding="utf-8") as handle:
    json.dump(response["result"]["structuredContent"], handle, indent=2)
    handle.write("\n")
PY
}

require_command git
require_command cargo
require_command npm
require_command docker
require_command curl
require_command python3

if [[ ! -d "$SERVER_REPO/.git" ]]; then
  printf 'PONTEMESH_SERVER_REPO is not a git repository: %s\n' "$SERVER_REPO" >&2
  exit 1
fi

mkdir -p "$ARTIFACT_DIR"
cat > "$COMPOSE_OVERRIDE" <<YAML
services:
  server:
    environment:
      PONTEMESH_PUBLIC_WEB_URL: http://127.0.0.1:$WEB_HOST_PORT
      PONTEMESH_PUBLIC_S3_URL: http://127.0.0.1:$S3_HOST_PORT
YAML

stage "update repositories"
if [[ "$PULL" == "1" ]]; then
  git -C "$SERVER_REPO" pull --ff-only
  git -C "$SDK_REPO" pull --ff-only
fi

stage "build server from repository"
(
  cd "$SERVER_REPO"
  ./scripts/check-migrations.sh
  npm install --prefix web
  npm run build --prefix web
  cargo build --release
)

stage "build sdk clients from repository"
(
  cd "$SDK_REPO"
  cargo build --release -p pontemesh-live-client
)

stage "start server compose stack"
if [[ "$RESET" == "1" ]]; then
  compose down --volumes --remove-orphans
fi
PONTEMESH_WEB_HOST_PORT="$WEB_HOST_PORT" \
PONTEMESH_S3_HOST_PORT="$S3_HOST_PORT" \
  compose up -d --build
COMPOSE_STARTED=1
WEB_URL="http://127.0.0.1:$WEB_HOST_PORT"
MCP_URL="$WEB_URL/mcp"
wait_for_http "$WEB_URL/api/setup/status" 120

stage "run setup agent"
compose exec -T server pontemesh-server setup-agent \
  --instance-name "Ponte Mesh SDK full stack" \
  --admin-username admin \
  --admin-password "$ADMIN_PASSWORD" \
  --http-port 8080 \
  --mcp-token-name "sdk-full-stack-agent" \
  --mcp-scopes read,write,admin \
  --connection-file /var/pontemesh_home/secrets/sdk-full-stack-agent-mcp.json \
  > "$ARTIFACT_DIR/setup-agent.json"
MCP_TOKEN="$(json_get "$ARTIFACT_DIR/setup-agent.json" "mcp.tokenSecret")"

stage "create bucket through mcp"
python3 - "$BUCKET" "$ARTIFACT_DIR/create-bucket-args.json" <<'PY'
import json
import sys
with open(sys.argv[2], "w", encoding="utf-8") as handle:
    json.dump({"bucket": sys.argv[1]}, handle)
PY
mcp_call pontemesh_create_bucket "$ARTIFACT_DIR/create-bucket-args.json" "$ARTIFACT_DIR/create-bucket-response.json"
structured_content "$ARTIFACT_DIR/create-bucket-response.json" "$ARTIFACT_DIR/create-bucket.json"

stage "generate and upload object through mcp"
python3 - "$OBJECT_BYTES" "$BUCKET" "$KEY" "$ARTIFACT_DIR/source-object.bin" "$ARTIFACT_DIR/put-object-args.json" "$ARTIFACT_DIR/object-meta.json" <<'PY'
import base64
import hashlib
import json
import sys

size = int(sys.argv[1])
bucket, key, object_path, args_path, meta_path = sys.argv[2:7]
seed = b"pontemesh-sdk-full-stack-agent-gate"
data = bytearray()
counter = 0
while len(data) < size:
    data.extend(hashlib.sha256(seed + counter.to_bytes(8, "big")).digest())
    counter += 1
data = bytes(data[:size])
with open(object_path, "wb") as handle:
    handle.write(data)
sha256 = hashlib.sha256(data).hexdigest()
with open(args_path, "w", encoding="utf-8") as handle:
    json.dump({
        "bucket": bucket,
        "key": key,
        "contentBase64": base64.b64encode(data).decode("ascii"),
        "contentType": "application/octet-stream",
    }, handle)
with open(meta_path, "w", encoding="utf-8") as handle:
    json.dump({"bucket": bucket, "key": key, "bytes": len(data), "sha256": sha256}, handle, indent=2)
    handle.write("\n")
PY
mcp_call pontemesh_put_base64_object "$ARTIFACT_DIR/put-object-args.json" "$ARTIFACT_DIR/put-object-response.json"
structured_content "$ARTIFACT_DIR/put-object-response.json" "$ARTIFACT_DIR/put-object.json"
OBJECT_SHA256="$(json_get "$ARTIFACT_DIR/object-meta.json" "sha256")"

stage "create sdk application credential through mcp"
python3 - "$ARTIFACT_DIR/create-application-args.json" <<'PY'
import json
import sys
with open(sys.argv[1], "w", encoding="utf-8") as handle:
    json.dump({
        "name": "sdk-full-stack-clients",
        "scopes": [
            "pontemesh:access-package:create",
            "pontemesh:manifest:read",
            "pontemesh:sources:read",
            "pontemesh:availability:read",
            "pontemesh:policies:read",
        ],
    }, handle)
PY
mcp_call pontemesh_create_application_credential "$ARTIFACT_DIR/create-application-args.json" "$ARTIFACT_DIR/create-application-response.json"
structured_content "$ARTIFACT_DIR/create-application-response.json" "$ARTIFACT_DIR/create-application.json"
APPLICATION_TOKEN="$(json_get "$ARTIFACT_DIR/create-application.json" "token")"

stage "run sdk client applications"
rm -rf "$ARTIFACT_DIR/clients"
mkdir -p "$ARTIFACT_DIR/clients"
failures=0
running=0
for index in $(seq 1 "$CLIENT_COUNT"); do
  (
    "$SDK_REPO/target/release/pontemesh-live-client" \
      --origin-url "$WEB_URL" \
      --application-token "$APPLICATION_TOKEN" \
      --bucket "$BUCKET" \
      --key "$KEY" \
      --destination "$ARTIFACT_DIR/clients/client-$index.bin" \
      --expected-sha256 "$OBJECT_SHA256" \
      > "$ARTIFACT_DIR/clients/client-$index.json"
  ) &
  running=$((running + 1))
  if (( running >= CLIENT_PARALLELISM )); then
    if ! wait -n; then
      failures=$((failures + 1))
    fi
    running=$((running - 1))
  fi
done
while (( running > 0 )); do
  if ! wait -n; then
    failures=$((failures + 1))
  fi
  running=$((running - 1))
done
if (( failures > 0 )); then
  printf '%s SDK client processes failed.\n' "$failures" >&2
  exit 1
fi

stage "summarize client results"
python3 - "$ARTIFACT_DIR/clients" "$CLIENT_COUNT" "$OBJECT_SHA256" "$ARTIFACT_DIR/summary.json" <<'PY'
import glob
import json
import os
import sys

clients_dir, expected_count, expected_sha, summary_path = sys.argv[1:5]
expected_count = int(expected_count)
items = []
for path in sorted(glob.glob(os.path.join(clients_dir, "client-*.json"))):
    with open(path, "r", encoding="utf-8") as handle:
        items.append(json.load(handle))
if len(items) != expected_count:
    raise SystemExit(f"expected {expected_count} client reports, found {len(items)}")
bad = [item for item in items if item.get("sha256") != expected_sha or not item.get("ok")]
if bad:
    raise SystemExit(f"{len(bad)} client reports failed validation")
summary = {
    "ok": True,
    "clientCount": len(items),
    "expectedSha256": expected_sha,
    "totalBytes": sum(item["bytes"] for item in items),
    "totalElapsedMs": sum(item["elapsedMs"] for item in items),
    "sourceBytes": {
        "peer": sum(item["summary"]["bytesFromPeer"] for item in items),
        "replica": sum(item["summary"]["bytesFromReplica"] for item in items),
        "origin": sum(item["summary"]["bytesFromOrigin"] for item in items),
    },
}
with open(summary_path, "w", encoding="utf-8") as handle:
    json.dump(summary, handle, indent=2)
    handle.write("\n")
print(json.dumps(summary, indent=2))
PY

printf '\nFull stack SDK/server gate passed.\n'
printf 'Artifacts: %s\n' "$(artifact_display_path "$ARTIFACT_DIR")"
