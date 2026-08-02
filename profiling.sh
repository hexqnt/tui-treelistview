#!/usr/bin/env bash

# perf-профилировка Criterion-бенчмарков из benches/perf.
#
# Профиль Cargo `profiling` сохраняет release-оптимизации и добавляет отладочную
# информацию. Скрипт также включает frame pointers, чтобы perf восстанавливал
# стеки оптимизированного кода.

set -Eeuo pipefail

readonly PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly PROFILE_ROOT="${PROFILE_DIR:-$PROJECT_DIR/target/profiles}"
readonly PROFILE_TIME="${PROFILE_TIME:-10}"
readonly PERF_EVENTS="${PERF_EVENTS:-task-clock,context-switches,cpu-migrations,page-faults,cycles,instructions,branches,branch-misses,cache-references,cache-misses}"
readonly PERF_STAT_REPEATS="${PERF_STAT_REPEATS:-3}"
readonly PERF_RECORD_FREQUENCY="${PERF_RECORD_FREQUENCY:-499}"

usage() {
    cat <<'EOF'
Usage:
  ./profiling.sh [options] <benchmark-filter>
  ./profiling.sh --list [options]

Options:
      --features <list>       Cargo feature list
      --all-features          Build with all Cargo features
      --exact                 Require an exact benchmark name
      --record-only           Collect call stacks only
      --stat-only             Collect hardware counters only
      --build-only            Build the profiling binary without running perf
      --list                  List available benchmarks
  -h, --help                  Show this help

Environment:
  PROFILE_DIR                 Output root (default: target/profiles)
  PROFILE_TIME                Seconds per matched benchmark (default: 10)
  PERF_STAT_REPEATS           perf stat repeat count (default: 3)
  PERF_RECORD_FREQUENCY       perf record sampling frequency (default: 499)
  PERF_EVENTS                 Comma-separated perf stat events

Examples:
  ./profiling.sh --list
  ./profiling.sh --exact projection/rebuild/balanced/100000
  PROFILE_TIME=5 ./profiling.sh 'marks/rebuild|interaction/deep_chain'
  ./profiling.sh --all-features --stat-only render/end_to_end

The filter is a regular expression understood by Criterion. Each matched benchmark
runs for PROFILE_TIME seconds once under perf record and PERF_STAT_REPEATS times
under perf stat.
EOF
}

require_option_value() {
    local option="$1"
    local value="${2-}"
    if [[ -z "$value" ]]; then
        echo "error: $option requires a value" >&2
        usage >&2
        exit 2
    fi
}

benchmark_filter=""
features=""
all_features=false
exact=false
record=true
stat=true
build_only=false
list=false

while (($# > 0)); do
    case "$1" in
        --features)
            require_option_value "$1" "${2-}"
            features="$2"
            shift 2
            ;;
        --all-features)
            all_features=true
            shift
            ;;
        --exact)
            exact=true
            shift
            ;;
        --record-only)
            record=true
            stat=false
            shift
            ;;
        --stat-only)
            record=false
            stat=true
            shift
            ;;
        --build-only)
            build_only=true
            shift
            ;;
        --list)
            list=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            if (($# > 1)); then
                echo "error: expected at most one benchmark filter" >&2
                usage >&2
                exit 2
            fi
            benchmark_filter="${1-}"
            break
            ;;
        -*)
            echo "error: unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
        *)
            if [[ -n "$benchmark_filter" ]]; then
                echo "error: expected exactly one benchmark filter" >&2
                usage >&2
                exit 2
            fi
            benchmark_filter="$1"
            shift
            ;;
    esac
done

if [[ -z "$benchmark_filter" && "$list" == false && "$build_only" == false ]]; then
    echo "error: a benchmark filter is required; use --list to see benchmark names" >&2
    usage >&2
    exit 2
fi
if [[ ! "$PROFILE_TIME" =~ ^[1-9][0-9]*$ ]]; then
    echo "error: PROFILE_TIME must be a positive integer" >&2
    exit 2
fi
if [[ ! "$PERF_STAT_REPEATS" =~ ^[1-9][0-9]*$ ]]; then
    echo "error: PERF_STAT_REPEATS must be a positive integer" >&2
    exit 2
fi
if [[ ! "$PERF_RECORD_FREQUENCY" =~ ^[1-9][0-9]*$ ]]; then
    echo "error: PERF_RECORD_FREQUENCY must be a positive integer" >&2
    exit 2
fi

declare -a cargo_command=(
    cargo bench
    --profile profiling
    --bench perf
    --no-run
    --message-format json-render-diagnostics
)
if [[ -n "$features" ]]; then
    cargo_command+=(--features "$features")
fi
if [[ "$all_features" == true ]]; then
    cargo_command+=(--all-features)
fi

profiling_rustflags="-C force-frame-pointers=yes"
if [[ -n "${RUSTFLAGS-}" ]]; then
    profiling_rustflags="$RUSTFLAGS $profiling_rustflags"
fi

cd -- "$PROJECT_DIR"
echo "Building optimized profiling benchmark: perf"
binary="$({
    env "RUSTFLAGS=$profiling_rustflags" "${cargo_command[@]}"
} | while IFS= read -r artifact; do
    if [[ "$artifact" == *'"kind":["bench"]'* \
        && "$artifact" == *'"name":"perf"'* \
        && "$artifact" =~ \"executable\":\"([^\"]+)\" ]]; then
        printf '%s\n' "${BASH_REMATCH[1]}"
    fi
done | tail -n 1)"
if [[ -z "$binary" || ! -x "$binary" ]]; then
    echo "error: Cargo did not report an executable perf benchmark" >&2
    exit 1
fi
readonly BINARY="$binary"

declare -a filter_arguments=()
if [[ -n "$benchmark_filter" ]]; then
    filter_arguments+=("$benchmark_filter")
fi
if [[ "$exact" == true ]]; then
    filter_arguments+=(--exact)
fi

if [[ "$list" == true ]]; then
    "$BINARY" "${filter_arguments[@]}" --list
    exit 0
fi
if [[ "$build_only" == true ]]; then
    echo "Built: $BINARY"
    exit 0
fi
if ! command -v perf >/dev/null 2>&1; then
    echo "error: perf is not installed or is not in PATH" >&2
    exit 1
fi

selected_benchmarks="$("$BINARY" "${filter_arguments[@]}" --list)"
if [[ "$selected_benchmarks" != *': benchmark'* ]]; then
    echo "error: the filter did not match any benchmarks: $benchmark_filter" >&2
    exit 2
fi
echo "Selected benchmarks:"
printf '%s\n' "$selected_benchmarks"

profile_name="${benchmark_filter//[^[:alnum:]_.-]/_}"
if [[ "$exact" == true ]]; then
    profile_name="exact-$profile_name"
fi
readonly PROFILE_OUTPUT_DIR="$PROFILE_ROOT/$profile_name"
mkdir -p -- "$PROFILE_OUTPUT_DIR"

declare -a benchmark_command=(
    "$BINARY"
    --bench
    "${filter_arguments[@]}"
    --profile-time "$PROFILE_TIME"
    --noplot
    --quiet
)

rotate_previous_output() {
    local output="$1"
    if [[ -e "$output" ]]; then
        mv -f -- "$output" "$output.old"
    fi
}

if [[ "$record" == true ]]; then
    readonly RECORD_OUTPUT="$PROFILE_OUTPUT_DIR/perf.data"
    rotate_previous_output "$RECORD_OUTPUT"
    echo "Recording call stacks for ${PROFILE_TIME}s per benchmark"
    perf record \
        --freq "$PERF_RECORD_FREQUENCY" \
        --event cycles:u \
        --call-graph fp \
        --output "$RECORD_OUTPUT" \
        -- "${benchmark_command[@]}"
fi

if [[ "$stat" == true ]]; then
    readonly STAT_OUTPUT="$PROFILE_OUTPUT_DIR/perf-stat.txt"
    rotate_previous_output "$STAT_OUTPUT"
    echo "Collecting counters ($PERF_STAT_REPEATS repeats, ${PROFILE_TIME}s per benchmark)"
    perf stat \
        --repeat "$PERF_STAT_REPEATS" \
        --event "$PERF_EVENTS" \
        --output "$STAT_OUTPUT" \
        -- "${benchmark_command[@]}"
fi

echo "Profiles written to: $PROFILE_OUTPUT_DIR"
if [[ "$record" == true ]]; then
    echo "Inspect call stacks with: perf report --input $RECORD_OUTPUT"
fi
