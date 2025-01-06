#!/usr/bin/env python3

import json
import sys
import os
import matplotlib.pyplot as plt
import seaborn as sns
import pandas as pd
import re

def parse_latency(lat_str):
    """Convert latency strings to milliseconds."""
    if 'us' in lat_str:
        return float(lat_str.replace('us', '')) / 1000
    elif 'ms' in lat_str:
        return float(lat_str.replace('ms', ''))
    elif 's' in lat_str:
        return float(lat_str.replace('s', '')) * 1000
    return float(lat_str)

def extract_number(s):
    """Extract the first number from a string."""
    match = re.search(r'\d+', s)
    return int(match.group()) if match else 0

def prepare_data(results):
    """Convert results to a pandas DataFrame."""
    data = []
    for result in results:
        if 'Basic HTTP' in result['description'] or 'Payload' in result['description']:
            row = {
                'proxy': result['proxy'],
                'connections': int(result['connections']),
                'throughput': float(result['throughput']),
                'latency_p50': parse_latency(result['latency']['p50']),
                'latency_p99': parse_latency(result['latency']['p99']),
                'test_type': 'Basic HTTP' if 'Basic HTTP' in result['description'] else 'Payload',
                'payload_size': extract_number(result['endpoint']) if 'size' in result['endpoint'] else 0
            }
            data.append(row)
    return pd.DataFrame(data)

def setup_plot_style():
    """Set up the plot style."""
    plt.style.use('seaborn')
    sns.set_palette("husl")
    plt.rcParams['figure.figsize'] = [12, 6]
    plt.rcParams['figure.dpi'] = 100

def plot_throughput_vs_concurrency(df, output_dir):
    """Plot throughput vs concurrency for basic HTTP tests."""
    plt.figure()
    basic_http = df[df['test_type'] == 'Basic HTTP']
    sns.lineplot(data=basic_http, x='connections', y='throughput', hue='proxy', marker='o')
    plt.xscale('log')
    plt.title('Throughput vs Concurrency')
    plt.xlabel('Connections')
    plt.ylabel('Requests/sec')
    plt.grid(True)
    plt.savefig(os.path.join(output_dir, 'throughput_vs_concurrency.png'))
    plt.close()

def plot_latency_vs_concurrency(df, output_dir):
    """Plot latency vs concurrency for basic HTTP tests."""
    plt.figure()
    basic_http = df[df['test_type'] == 'Basic HTTP']
    sns.lineplot(data=basic_http, x='connections', y='latency_p50', hue='proxy', marker='o', linestyle='-', label='p50')
    sns.lineplot(data=basic_http, x='connections', y='latency_p99', hue='proxy', marker='s', linestyle='--', label='p99')
    plt.xscale('log')
    plt.yscale('log')
    plt.title('Latency vs Concurrency')
    plt.xlabel('Connections')
    plt.ylabel('Latency (ms)')
    plt.grid(True)
    plt.savefig(os.path.join(output_dir, 'latency_vs_concurrency.png'))
    plt.close()

def plot_throughput_vs_payload(df, output_dir):
    """Plot throughput vs payload size."""
    plt.figure()
    payload_tests = df[df['test_type'] == 'Payload']
    sns.lineplot(data=payload_tests, x='payload_size', y='throughput', hue='proxy', marker='o')
    plt.title('Throughput vs Payload Size')
    plt.xlabel('Payload Size (KB)')
    plt.ylabel('Requests/sec')
    plt.grid(True)
    plt.savefig(os.path.join(output_dir, 'throughput_vs_payload.png'))
    plt.close()

def plot_latency_vs_payload(df, output_dir):
    """Plot latency vs payload size."""
    plt.figure()
    payload_tests = df[df['test_type'] == 'Payload']
    sns.lineplot(data=payload_tests, x='payload_size', y='latency_p50', hue='proxy', marker='o', linestyle='-', label='p50')
    sns.lineplot(data=payload_tests, x='payload_size', y='latency_p99', hue='proxy', marker='s', linestyle='--', label='p99')
    plt.title('Latency vs Payload Size')
    plt.xlabel('Payload Size (KB)')
    plt.ylabel('Latency (ms)')
    plt.grid(True)
    plt.savefig(os.path.join(output_dir, 'latency_vs_payload.png'))
    plt.close()

def main():
    if len(sys.argv) != 2:
        print("Usage: generate_plots.py <data_file>")
        sys.exit(1)

    data_file = sys.argv[1]
    with open(data_file) as f:
        data = json.load(f)

    # Create plots directory
    plots_dir = os.path.join(os.path.dirname(data_file), '..', 'plots')
    os.makedirs(plots_dir, exist_ok=True)

    # Prepare data
    df = prepare_data(data['results'])

    # Set up plot style
    setup_plot_style()

    # Generate plots
    plot_throughput_vs_concurrency(df, plots_dir)
    plot_latency_vs_concurrency(df, plots_dir)
    plot_throughput_vs_payload(df, plots_dir)
    plot_latency_vs_payload(df, plots_dir)

if __name__ == '__main__':
    main() 