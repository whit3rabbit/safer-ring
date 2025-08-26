#!/bin/bash

# Safer-Ring Performance Benchmark Runner
# This script runs all benchmarks and generates performance reports

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BENCHMARK_DIR="target/criterion"
FLAMEGRAPH_DIR="target/flamegraphs"
REPORT_DIR="target/benchmark_reports"

echo -e "${BLUE}Safer-Ring Performance Benchmark Suite${NC}"
echo "========================================"

# Create output directories
mkdir -p "$FLAMEGRAPH_DIR"
mkdir -p "$REPORT_DIR"

# Check if we're on Linux (required for io_uring)
if [[ "$OSTYPE" != "linux-gnu"* ]]; then
    echo -e "${YELLOW}Warning: io_uring is only available on Linux. Some benchmarks may not run.${NC}"
fi

# Function to run a benchmark with error handling
run_benchmark() {
    local bench_name=$1
    local description=$2
    
    echo -e "\n${GREEN}Running $description...${NC}"
    
    if cargo bench --bench "$bench_name" 2>&1 | tee "$REPORT_DIR/${bench_name}.log"; then
        echo -e "${GREEN}✓ $description completed successfully${NC}"
    else
        echo -e "${RED}✗ $description failed${NC}"
        return 1
    fi
}

# Function to check system requirements
check_requirements() {
    echo -e "\n${BLUE}Checking system requirements...${NC}"
    
    # Check for required tools
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Error: cargo not found${NC}"
        exit 1
    fi
    
    # Check for perf (for flamegraphs)
    if ! command -v perf &> /dev/null; then
        echo -e "${YELLOW}Warning: perf not found. Flamegraphs may not work.${NC}"
    fi
    
    # Check kernel version for io_uring support
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        kernel_version=$(uname -r | cut -d. -f1-2)
        required_version="5.1"
        
        if ! printf '%s\n%s\n' "$required_version" "$kernel_version" | sort -V -C; then
            echo -e "${YELLOW}Warning: Kernel version $kernel_version may not fully support io_uring (requires 5.1+)${NC}"
        else
            echo -e "${GREEN}✓ Kernel version $kernel_version supports io_uring${NC}"
        fi
    fi
    
    echo -e "${GREEN}✓ System requirements check completed${NC}"
}

# Function to set up performance environment
setup_perf_env() {
    echo -e "\n${BLUE}Setting up performance environment...${NC}"
    
    # Set CPU governor to performance mode (if available)
    if [[ -f /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor ]]; then
        echo -e "${YELLOW}Note: Consider setting CPU governor to 'performance' for consistent results:${NC}"
        echo "  sudo cpupower frequency-set -g performance"
    fi
    
    # Disable CPU frequency scaling (if available)
    if [[ -f /proc/sys/kernel/nmi_watchdog ]]; then
        echo -e "${YELLOW}Note: Consider disabling NMI watchdog for better performance:${NC}"
        echo "  echo 0 | sudo tee /proc/sys/kernel/nmi_watchdog"
    fi
    
    # Set process priority
    echo -e "${GREEN}Setting high process priority...${NC}"
    renice -n -10 $$ 2>/dev/null || echo -e "${YELLOW}Warning: Could not set high priority${NC}"
}

# Function to generate summary report
generate_summary() {
    echo -e "\n${BLUE}Generating benchmark summary...${NC}"
    
    local summary_file="$REPORT_DIR/summary.md"
    
    cat > "$summary_file" << EOF
# Safer-Ring Benchmark Summary

Generated on: $(date)
System: $(uname -a)
Rust version: $(rustc --version)

## Benchmark Results

EOF
    
    # Add results from each benchmark
    for bench in micro_benchmarks macro_benchmarks memory_benchmarks numa_benchmarks; do
        if [[ -f "$REPORT_DIR/${bench}.log" ]]; then
            echo "### $bench" >> "$summary_file"
            echo '```' >> "$summary_file"
            tail -n 20 "$REPORT_DIR/${bench}.log" >> "$summary_file"
            echo '```' >> "$summary_file"
            echo "" >> "$summary_file"
        fi
    done
    
    echo -e "${GREEN}✓ Summary report generated: $summary_file${NC}"
}

# Function to open results
open_results() {
    echo -e "\n${BLUE}Opening benchmark results...${NC}"
    
    # Try to open Criterion HTML report
    if [[ -f "$BENCHMARK_DIR/report/index.html" ]]; then
        if command -v xdg-open &> /dev/null; then
            xdg-open "$BENCHMARK_DIR/report/index.html" &
        elif command -v open &> /dev/null; then
            open "$BENCHMARK_DIR/report/index.html" &
        else
            echo -e "${YELLOW}HTML report available at: $BENCHMARK_DIR/report/index.html${NC}"
        fi
    fi
    
    echo -e "${GREEN}Results available in:${NC}"
    echo "  - HTML reports: $BENCHMARK_DIR/report/"
    echo "  - Flamegraphs: $FLAMEGRAPH_DIR/"
    echo "  - Raw logs: $REPORT_DIR/"
}

# Main execution
main() {
    local run_all=true
    local benchmarks=()
    
    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --micro)
                benchmarks+=("micro_benchmarks")
                run_all=false
                shift
                ;;
            --macro)
                benchmarks+=("macro_benchmarks")
                run_all=false
                shift
                ;;
            --memory)
                benchmarks+=("memory_benchmarks")
                run_all=false
                shift
                ;;
            --numa)
                benchmarks+=("numa_benchmarks")
                run_all=false
                shift
                ;;
            --help|-h)
                echo "Usage: $0 [--micro] [--macro] [--memory] [--numa] [--help]"
                echo ""
                echo "Options:"
                echo "  --micro   Run micro-benchmarks only"
                echo "  --macro   Run macro-benchmarks only"
                echo "  --memory  Run memory benchmarks only"
                echo "  --numa    Run NUMA benchmarks only"
                echo "  --help    Show this help message"
                echo ""
                echo "If no specific benchmark is specified, all benchmarks will be run."
                exit 0
                ;;
            *)
                echo -e "${RED}Unknown option: $1${NC}"
                exit 1
                ;;
        esac
    done
    
    # Set default benchmarks if none specified
    if [[ "$run_all" == true ]]; then
        benchmarks=("micro_benchmarks" "macro_benchmarks" "memory_benchmarks" "numa_benchmarks")
    fi
    
    # Run setup
    check_requirements
    setup_perf_env
    
    # Build benchmarks first
    echo -e "\n${BLUE}Building benchmarks...${NC}"
    if ! cargo build --release --benches; then
        echo -e "${RED}Failed to build benchmarks${NC}"
        exit 1
    fi
    
    # Run selected benchmarks
    local failed_benchmarks=()
    
    for bench in "${benchmarks[@]}"; do
        case $bench in
            "micro_benchmarks")
                run_benchmark "$bench" "Micro-benchmarks (individual operations)" || failed_benchmarks+=("$bench")
                ;;
            "macro_benchmarks")
                run_benchmark "$bench" "Macro-benchmarks (vs raw io_uring)" || failed_benchmarks+=("$bench")
                ;;
            "memory_benchmarks")
                run_benchmark "$bench" "Memory usage profiling" || failed_benchmarks+=("$bench")
                ;;
            "numa_benchmarks")
                run_benchmark "$bench" "NUMA-aware optimizations" || failed_benchmarks+=("$bench")
                ;;
        esac
    done
    
    # Generate summary
    generate_summary
    
    # Report results
    echo -e "\n${BLUE}Benchmark Results Summary${NC}"
    echo "========================="
    
    local successful=$((${#benchmarks[@]} - ${#failed_benchmarks[@]}))
    echo -e "${GREEN}Successful: $successful/${#benchmarks[@]}${NC}"
    
    if [[ ${#failed_benchmarks[@]} -gt 0 ]]; then
        echo -e "${RED}Failed: ${failed_benchmarks[*]}${NC}"
    fi
    
    # Open results
    open_results
    
    echo -e "\n${GREEN}Benchmark suite completed!${NC}"
    
    # Exit with error if any benchmarks failed
    if [[ ${#failed_benchmarks[@]} -gt 0 ]]; then
        exit 1
    fi
}

# Run main function with all arguments
main "$@"