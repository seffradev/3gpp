#!/usr/bin/env bash
#
# Orchestrates the full GTP proxy load-test plan:
#   1. Closed-loop coarse concurrency sweep
#   2. Closed-loop fine sweep (you fill in after inspecting stage 1)
#   3. Open-loop coarse rate sweep
#   4. Open-loop fine sweep (you fill in after inspecting stage 3)
#   5. Drift check: repeat the lowest closed-loop point
#   6. Soak test: moderate concurrency, sessions created+deleted continuously
#   7. Accumulation test: moderate concurrency, sessions never deleted
#
# Each stage writes its own timestamped CSV via --metrics-file, and a
# manifest.tsv row recording exactly what was run and where the output went.
#
# Usage:
#   ./run_test_plan.sh                     # run everything
#   ./run_test_plan.sh closed_coarse        # run just one phase
#   ./run_test_plan.sh closed_coarse open_coarse   # run a subset, in order
#
# Edit the CONFIG section below before running.

set -euo pipefail

# ============================== CONFIG ==============================

BINARY="${BINARY:-./target/release/load}"
TARGET="${TARGET:-127.0.0.1:2123}"
BIND="${BIND:-0.0.0.0:2123}"
IMSI_BASE="${IMSI_BASE:-001010000000001}"
TIMEOUT_MS="${TIMEOUT_MS:-1000}"
METRICS_INTERVAL_SECS="${METRICS_INTERVAL_SECS:-5}"
COOLDOWN_SECS="${COOLDOWN_SECS:-20}"          # pause between stages, lets any transient load drain
OUT_DIR="${OUT_DIR:-./testplan-results/$(date +%Y%m%d-%H%M%S)}"

# Phase durations
COARSE_DURATION_SECS="${COARSE_DURATION_SECS:-120}"   # 2 min
FINE_DURATION_SECS="${FINE_DURATION_SECS:-300}"        # 5 min
DRIFT_DURATION_SECS="${DRIFT_DURATION_SECS:-120}"      # 2 min
SOAK_DURATION_SECS="${SOAK_DURATION_SECS:-7200}"       # 2 hr
ACCUM_DURATION_SECS="${ACCUM_DURATION_SECS:-3600}"     # 1 hr

# Coarse sweep points
CLOSED_COARSE_CONCURRENCY=(1 10 50 100 500)
OPEN_COARSE_RATE=(1 10 100 1000 10000)

# Fine sweep points — fill these in AFTER inspecting the coarse-phase plots,
# bracketing wherever throughput plateaus / p99 starts climbing. Left empty
# by default so `fine` phases are a no-op until you've set them.
CLOSED_FINE_CONCURRENCY=()   # e.g. (150 200 250 300 350)
OPEN_FINE_RATE=()            # e.g. (400 600 800 1000 1200)

# Soak / accumulation concurrency — pick something comfortably under whatever
# capacity you found in the closed-loop sweep, e.g. half of it.
SOAK_CONCURRENCY="${SOAK_CONCURRENCY:-100}"
ACCUM_CONCURRENCY="${ACCUM_CONCURRENCY:-100}"

# ======================================================================

mkdir -p "$OUT_DIR"
MANIFEST="$OUT_DIR/manifest.tsv"
echo -e "stage\tparam\tvalue\tduration_secs\tcsv_file\tstarted_at\tfinished_at" > "$MANIFEST"

log() { echo "[$(date '+%H:%M:%S')] $*"; }

cooldown() {
    if [[ "$COOLDOWN_SECS" -gt 0 ]]; then
        log "cooldown ${COOLDOWN_SECS}s..."
        sleep "$COOLDOWN_SECS"
    fi
}

record_manifest() {
    local stage="$1" param="$2" value="$3" duration="$4" csv="$5" started="$6" finished="$7"
    echo -e "${stage}\t${param}\t${value}\t${duration}\t${csv}\t${started}\t${finished}" >> "$MANIFEST"
}

run_closed() {
    local concurrency="$1" duration="$2" stage_tag="$3"
    local csv="$OUT_DIR/${stage_tag}_c${concurrency}.csv"
    log "closed-loop: concurrency=${concurrency} duration=${duration}s -> ${csv}"
    local started finished
    started="$(date -Iseconds)"
    "$BINARY" client \
        --target "$TARGET" --bind "$BIND" \
        --loop-mode closed --concurrency "$concurrency" \
        --duration-secs "$duration" --timeout-ms "$TIMEOUT_MS" \
        --imsi-base "$IMSI_BASE" --with-delete \
        --metrics-file "$csv" --metrics-interval-secs "$METRICS_INTERVAL_SECS" \
        2>&1 | tee "$OUT_DIR/${stage_tag}_c${concurrency}.log"
    finished="$(date -Iseconds)"
    record_manifest "$stage_tag" "concurrency" "$concurrency" "$duration" "$csv" "$started" "$finished"
    cooldown
}

run_open() {
    local rate="$1" duration="$2" stage_tag="$3"
    local csv="$OUT_DIR/${stage_tag}_r${rate}.csv"
    log "open-loop: rate=${rate}/s duration=${duration}s -> ${csv}"
    local started finished
    started="$(date -Iseconds)"
    "$BINARY" client \
        --target "$TARGET" --bind "$BIND" \
        --loop-mode open --rate "$rate" \
        --duration-secs "$duration" --timeout-ms "$TIMEOUT_MS" \
        --imsi-base "$IMSI_BASE" \
        --metrics-file "$csv" --metrics-interval-secs "$METRICS_INTERVAL_SECS" \
        2>&1 | tee "$OUT_DIR/${stage_tag}_r${rate}.log"
    finished="$(date -Iseconds)"
    record_manifest "$stage_tag" "rate" "$rate" "$duration" "$csv" "$started" "$finished"
    cooldown
}

phase_closed_coarse() {
    log "=== Phase: closed-loop coarse sweep ==="
    for c in "${CLOSED_COARSE_CONCURRENCY[@]}"; do
        run_closed "$c" "$COARSE_DURATION_SECS" "closed_coarse"
    done
}

phase_closed_fine() {
    if [[ ${#CLOSED_FINE_CONCURRENCY[@]} -eq 0 ]]; then
        log "=== Phase: closed-loop fine sweep -- SKIPPED (CLOSED_FINE_CONCURRENCY is empty; fill it in after reviewing coarse results) ==="
        return
    fi
    log "=== Phase: closed-loop fine sweep ==="
    for c in "${CLOSED_FINE_CONCURRENCY[@]}"; do
        run_closed "$c" "$FINE_DURATION_SECS" "closed_fine"
    done
}

phase_open_coarse() {
    log "=== Phase: open-loop coarse sweep ==="
    for r in "${OPEN_COARSE_RATE[@]}"; do
        run_open "$r" "$COARSE_DURATION_SECS" "open_coarse"
    done
}

phase_open_fine() {
    if [[ ${#OPEN_FINE_RATE[@]} -eq 0 ]]; then
        log "=== Phase: open-loop fine sweep -- SKIPPED (OPEN_FINE_RATE is empty; fill it in after reviewing coarse results) ==="
        return
    fi
    log "=== Phase: open-loop fine sweep ==="
    for r in "${OPEN_FINE_RATE[@]}"; do
        run_open "$r" "$FINE_DURATION_SECS" "open_fine"
    done
}

phase_drift_check() {
    log "=== Phase: drift check (repeat of lowest closed-loop point) ==="
    local baseline="${CLOSED_COARSE_CONCURRENCY[0]}"
    run_closed "$baseline" "$DRIFT_DURATION_SECS" "drift_check"
}

phase_soak() {
    log "=== Phase: soak test (concurrency=${SOAK_CONCURRENCY}, with-delete, ${SOAK_DURATION_SECS}s) ==="
    run_closed "$SOAK_CONCURRENCY" "$SOAK_DURATION_SECS" "soak"
}

phase_accumulation() {
    local csv="$OUT_DIR/accum_c${ACCUM_CONCURRENCY}.csv"
    log "=== Phase: accumulation test (concurrency=${ACCUM_CONCURRENCY}, no delete, ${ACCUM_DURATION_SECS}s) -> ${csv} ==="
    local started finished
    started="$(date -Iseconds)"
    "$BINARY" client \
        --target "$TARGET" --bind "$BIND" \
        --loop-mode closed --concurrency "$ACCUM_CONCURRENCY" \
        --duration-secs "$ACCUM_DURATION_SECS" --timeout-ms "$TIMEOUT_MS" \
        --imsi-base "$IMSI_BASE" \
        --metrics-file "$csv" --metrics-interval-secs "$METRICS_INTERVAL_SECS" \
        2>&1 | tee "$OUT_DIR/accum_c${ACCUM_CONCURRENCY}.log"
    finished="$(date -Iseconds)"
    record_manifest "accumulation" "concurrency" "$ACCUM_CONCURRENCY" "$ACCUM_DURATION_SECS" "$csv" "$started" "$finished"
    cooldown
}

# ============================== RUNNER ==============================

ALL_PHASES=(closed_coarse closed_fine open_coarse open_fine drift_check soak accumulation)

run_phase() {
    case "$1" in
        closed_coarse) phase_closed_coarse ;;
        closed_fine)   phase_closed_fine ;;
        open_coarse)   phase_open_coarse ;;
        open_fine)     phase_open_fine ;;
        drift_check)   phase_drift_check ;;
        soak)          phase_soak ;;
        accumulation)  phase_accumulation ;;
        *) log "unknown phase: $1 (valid: ${ALL_PHASES[*]})"; exit 1 ;;
    esac
}

log "test plan starting, results -> $OUT_DIR"
log "binary=$BINARY target=$TARGET bind=$BIND"

if [[ $# -eq 0 ]]; then
    for phase in "${ALL_PHASES[@]}"; do
        run_phase "$phase"
    done
else
    for phase in "$@"; do
        run_phase "$phase"
    done
fi

log "test plan complete. results in $OUT_DIR"
log "manifest: $MANIFEST"
