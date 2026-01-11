#!/bin/bash
# Quick Start Script for Covenant LLM Generation Testing
# This script helps you get started with testing the LLM generation system

set -e  # Exit on error

echo "=========================================="
echo "Covenant LLM Generation - Quick Start"
echo "=========================================="
echo ""

# Check Python version
echo "Checking Python version..."
python_version=$(python --version 2>&1 | awk '{print $2}')
echo "✓ Python $python_version"
echo ""

# Check if in correct directory
if [ ! -f "run_evaluation.py" ]; then
    echo "ERROR: Must run from llm-context directory"
    echo "Run: cd llm-context && ./quickstart.sh"
    exit 1
fi

# Function to check if package is installed
check_package() {
    python -c "import $1" 2>/dev/null
    return $?
}

# Check dependencies
echo "Checking dependencies..."
missing_deps=0

if ! check_package "anthropic"; then
    echo "✗ anthropic package not installed"
    missing_deps=1
else
    echo "✓ anthropic installed"
fi

if [ $missing_deps -eq 1 ]; then
    echo ""
    read -p "Install missing dependencies? (y/n) " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        pip install anthropic
        echo "✓ Dependencies installed"
    else
        echo "Continuing without installing dependencies (will use mock mode)"
    fi
fi
echo ""

# Check compiler
echo "Checking Covenant compiler..."
cd ..
if [ -f "target/debug/covenant-cli" ] || [ -f "target/debug/covenant-cli.exe" ]; then
    echo "✓ Compiler found"
else
    echo "✗ Compiler not built"
    read -p "Build compiler now? (y/n) " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Building compiler..."
        cargo build
        echo "✓ Compiler built"
    else
        echo "Continuing without compiler (validation will be limited)"
    fi
fi
cd llm-context
echo ""

# Check API key
echo "Checking API configuration..."
if [ -z "$ANTHROPIC_API_KEY" ]; then
    echo "✗ ANTHROPIC_API_KEY not set"
    echo ""
    echo "To use real API:"
    echo "  export ANTHROPIC_API_KEY=your_key_here"
    echo ""
    echo "For now, we'll use mock mode (no API calls)"
    USE_MOCK=1
else
    echo "✓ ANTHROPIC_API_KEY is set"
    USE_MOCK=0
fi
echo ""

# Interactive menu
echo "=========================================="
echo "What would you like to do?"
echo "=========================================="
echo ""
echo "1) Run quick test (5 tasks, mock mode)"
echo "2) Run sample test (20 tasks, mock mode)"
echo "3) Run sample test (20 tasks, REAL API - costs ~\$3-6)"
echo "4) Run full suite (100+ tasks, REAL API - costs ~\$15-30)"
echo "5) View test suite statistics"
echo "6) Analyze existing results"
echo "7) Exit"
echo ""

read -p "Enter choice (1-7): " choice

case $choice in
    1)
        echo ""
        echo "Running quick test (5 tasks, mock mode)..."
        python run_evaluation.py --sample 5 --verbose
        ;;
    2)
        echo ""
        echo "Running sample test (20 tasks, mock mode)..."
        python run_evaluation.py --sample 20 --verbose
        ;;
    3)
        if [ $USE_MOCK -eq 1 ]; then
            echo ""
            echo "ERROR: ANTHROPIC_API_KEY not set"
            echo "Run: export ANTHROPIC_API_KEY=your_key_here"
            exit 1
        fi
        echo ""
        echo "WARNING: This will make real API calls!"
        echo "Estimated cost: \$3-6"
        read -p "Continue? (yes/no): " confirm
        if [ "$confirm" == "yes" ]; then
            timestamp=$(date +%Y%m%d_%H%M%S)
            output="results_sample_${timestamp}.jsonl"
            echo ""
            echo "Running sample test (20 tasks, real API)..."
            python run_evaluation.py --provider anthropic --sample 20 --output "$output" --verbose
            echo ""
            echo "Generating analysis..."
            python run_evaluation.py --analyze "$output"
        else
            echo "Cancelled"
        fi
        ;;
    4)
        if [ $USE_MOCK -eq 1 ]; then
            echo ""
            echo "ERROR: ANTHROPIC_API_KEY not set"
            echo "Run: export ANTHROPIC_API_KEY=your_key_here"
            exit 1
        fi
        echo ""
        echo "WARNING: This will make 100+ real API calls!"
        echo "Estimated cost: \$15-30"
        read -p "Continue? (yes/no): " confirm
        if [ "$confirm" == "yes" ]; then
            timestamp=$(date +%Y%m%d_%H%M%S)
            output="results_full_${timestamp}.jsonl"
            echo ""
            echo "Running full suite (100+ tasks, real API)..."
            python run_evaluation.py --provider anthropic --output "$output"
            echo ""
            echo "Generating analysis..."
            python run_evaluation.py --analyze "$output"
        else
            echo "Cancelled"
        fi
        ;;
    5)
        echo ""
        echo "Test Suite Statistics:"
        echo "=========================================="
        python test_suite.py
        ;;
    6)
        echo ""
        # Find most recent results file
        latest=$(ls -t results_*.jsonl 2>/dev/null | head -1)
        if [ -z "$latest" ]; then
            echo "No results files found"
            echo "Run a test first (options 1-4)"
        else
            echo "Analyzing: $latest"
            python run_evaluation.py --analyze "$latest"
        fi
        ;;
    7)
        echo "Exiting"
        exit 0
        ;;
    *)
        echo "Invalid choice"
        exit 1
        ;;
esac

echo ""
echo "=========================================="
echo "Done!"
echo "=========================================="
