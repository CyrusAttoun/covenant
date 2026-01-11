"""
Evaluation Runner for Covenant LLM Generation

Runs the full test suite and generates analysis reports.
"""

import argparse
import json
from pathlib import Path
from datetime import datetime

from test_suite import create_test_suite, get_test_suite_sample
from generation_harness import (
    run_test_suite, print_summary, GenerationMetrics, ModelProvider
)


def analyze_results(results_file: Path):
    """
    Analyze results from a test run and generate detailed report.

    Args:
        results_file: Path to JSONL results file
    """
    results = []
    with open(results_file, 'r', encoding='utf-8') as f:
        for line in f:
            if line.strip():
                results.append(json.loads(line))

    total = len(results)
    if total == 0:
        print("No results to analyze")
        return

    # Overall metrics
    first_pass = sum(1 for r in results if r['first_pass_success'])
    final = sum(1 for r in results if r['final_success'])

    total_prompt = sum(r['total_prompt_tokens'] for r in results)
    total_completion = sum(r['total_completion_tokens'] for r in results)
    total_cost = sum(r['total_cost_usd'] for r in results)
    total_time = sum(r['total_duration_ms'] for r in results)

    avg_attempts = sum(r['total_attempts'] for r in results) / total
    avg_cost = total_cost / total
    avg_time = total_time / total

    # Print report
    print("\n" + "="*80)
    print("EVALUATION REPORT")
    print("="*80)
    print(f"Date: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"Results file: {results_file}")
    print()

    print("OVERALL METRICS")
    print("-"*80)
    print(f"Total tasks: {total}")
    print(f"First-pass success: {first_pass}/{total} ({first_pass/total*100:.1f}%)")
    print(f"Final success: {final}/{total} ({final/total*100:.1f}%)")
    print(f"Improvement: {final - first_pass} tasks (+{(final-first_pass)/total*100:.1f}%)")
    print()

    print("RESOURCE USAGE")
    print("-"*80)
    print(f"Total prompt tokens: {total_prompt:,}")
    print(f"Total completion tokens: {total_completion:,}")
    print(f"Total tokens: {total_prompt + total_completion:,}")
    print(f"Total cost: ${total_cost:.2f}")
    print(f"Total time: {total_time/1000:.1f}s")
    print()

    print("AVERAGES PER TASK")
    print("-"*80)
    print(f"Average attempts: {avg_attempts:.2f}")
    print(f"Average cost: ${avg_cost:.4f}")
    print(f"Average time: {avg_time:.0f}ms")
    print(f"Average tokens: {(total_prompt + total_completion)/total:.0f}")
    print()

    # Success rate by task type
    print("SUCCESS RATE BY TASK TYPE")
    print("-"*80)
    from collections import defaultdict
    by_type = defaultdict(lambda: {'total': 0, 'first': 0, 'final': 0})

    for r in results:
        task_type = r['task_type']
        by_type[task_type]['total'] += 1
        if r['first_pass_success']:
            by_type[task_type]['first'] += 1
        if r['final_success']:
            by_type[task_type]['final'] += 1

    for task_type in sorted(by_type.keys()):
        stats = by_type[task_type]
        total_t = stats['total']
        first_t = stats['first']
        final_t = stats['final']
        print(f"{task_type:20s}: {final_t:3d}/{total_t:3d} ({final_t/total_t*100:5.1f}%) "
              f"[first: {first_t/total_t*100:5.1f}%]")
    print()

    # Error analysis
    print("ERROR ANALYSIS")
    print("-"*80)
    from collections import Counter
    all_errors = []
    for r in results:
        all_errors.extend(r.get('error_codes', []))

    if all_errors:
        print("Most common errors:")
        for code, count in Counter(all_errors).most_common(15):
            print(f"  {code:20s}: {count:3d} ({count/len([r for r in results if not r['final_success']])*100:.1f}% of failures)")
    else:
        print("No errors (all tasks succeeded!)")
    print()

    # Cost breakdown
    print("COST BREAKDOWN")
    print("-"*80)

    # By complexity (measured by attempts needed)
    simple = [r for r in results if r['total_attempts'] == 1]
    medium = [r for r in results if 1 < r['total_attempts'] <= 2]
    complex_tasks = [r for r in results if r['total_attempts'] > 2]

    print(f"Simple (1 attempt):  {len(simple):3d} tasks, ${sum(r['total_cost_usd'] for r in simple):.2f} total, "
          f"${sum(r['total_cost_usd'] for r in simple)/len(simple):.4f} avg" if simple else "")
    print(f"Medium (2 attempts): {len(medium):3d} tasks, ${sum(r['total_cost_usd'] for r in medium):.2f} total, "
          f"${sum(r['total_cost_usd'] for r in medium)/len(medium):.4f} avg" if medium else "")
    print(f"Complex (3+ attempts): {len(complex_tasks):3d} tasks, ${sum(r['total_cost_usd'] for r in complex_tasks):.2f} total, "
          f"${sum(r['total_cost_usd'] for r in complex_tasks)/len(complex_tasks):.4f} avg" if complex_tasks else "")
    print()

    # Failure examples
    failures = [r for r in results if not r['final_success']]
    if failures:
        print(f"FAILURE EXAMPLES (showing first 5 of {len(failures)})")
        print("-"*80)
        for r in failures[:5]:
            print(f"{r['task_id']}:")
            print(f"  Type: {r['task_type']}")
            print(f"  Attempts: {r['total_attempts']}")
            print(f"  Errors: {', '.join(r.get('error_codes', ['unknown']))}")
            print()

    # Export summary
    summary_file = results_file.with_suffix('.summary.json')
    summary = {
        'date': datetime.now().isoformat(),
        'total_tasks': total,
        'first_pass_success': first_pass,
        'first_pass_rate': first_pass / total,
        'final_success': final,
        'final_success_rate': final / total,
        'avg_attempts': avg_attempts,
        'avg_cost_usd': avg_cost,
        'avg_time_ms': avg_time,
        'total_cost_usd': total_cost,
        'total_time_ms': total_time,
        'by_type': {
            task_type: {
                'total': stats['total'],
                'first_pass_success': stats['first'],
                'final_success': stats['final'],
                'first_pass_rate': stats['first'] / stats['total'],
                'final_success_rate': stats['final'] / stats['total'],
            }
            for task_type, stats in by_type.items()
        },
        'top_errors': Counter(all_errors).most_common(10),
    }

    with open(summary_file, 'w', encoding='utf-8') as f:
        json.dump(summary, f, indent=2)

    print(f"Summary exported to: {summary_file}")
    print("="*80)


def main():
    parser = argparse.ArgumentParser(description="Run Covenant LLM generation evaluation")
    parser.add_argument('--sample', type=int, help="Run on random sample of N tasks")
    parser.add_argument('--category', type=str, help="Run only tasks of specific category")
    parser.add_argument('--output', type=str, help="Output file for results (JSONL)")
    parser.add_argument('--analyze', type=str, help="Analyze existing results file")
    parser.add_argument('--verbose', '-v', action='store_true', help="Verbose output")
    parser.add_argument('--provider', type=str, default='mock',
                       choices=['anthropic', 'openai', 'mock'],
                       help="LLM provider (default: mock)")

    args = parser.parse_args()

    # Analysis mode
    if args.analyze:
        analyze_results(Path(args.analyze))
        return

    # Generation mode
    print("Covenant LLM Generation Evaluation")
    print("="*80)

    # Select tasks
    if args.sample:
        print(f"Running sample of {args.sample} tasks")
        tasks = get_test_suite_sample(args.sample)
    elif args.category:
        from example_selector import TaskType
        task_type = TaskType(args.category)
        from test_suite import get_test_suite_by_category
        tasks = get_test_suite_by_category(task_type)
        print(f"Running {len(tasks)} tasks from category: {args.category}")
    else:
        tasks = create_test_suite()
        print(f"Running full test suite: {len(tasks)} tasks")

    # Set output file
    if args.output:
        output_file = args.output
    else:
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        output_file = f"results_{timestamp}.jsonl"

    print(f"Provider: {args.provider}")
    print(f"Output: {output_file}")
    print()

    if args.provider != 'mock':
        print("WARNING: This will make real API calls and incur costs!")
        print(f"Estimated cost: ${len(tasks) * 0.15:.2f} - ${len(tasks) * 0.30:.2f}")
        response = input("Continue? (yes/no): ")
        if response.lower() not in ['yes', 'y']:
            print("Aborted")
            return

    # Run evaluation
    print("\nStarting evaluation...")
    print("="*80)

    results = run_test_suite(tasks, output_file=output_file, verbose=args.verbose)

    # Print summary
    print_summary(results)

    # Generate detailed analysis
    print("\nGenerating detailed analysis...")
    analyze_results(Path(output_file))


if __name__ == "__main__":
    main()
