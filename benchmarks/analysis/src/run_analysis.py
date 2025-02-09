import argparse
import os
from pathlib import Path
import pathlib
import sys

import pandas
import seaborn as sns
import matplotlib.pyplot as plt

def read_criterion_raws(target_dir: str) -> pandas.DataFrame:
    def onerror(err: OSError):
        raise err

    dataframes = []
    for (root, _, files) in os.walk(target_dir, onerror=onerror):
        RAW_FILE_NAME: str = "raw.csv"
        if RAW_FILE_NAME in files:
            path = Path(root)
            raw_path = path.joinpath(RAW_FILE_NAME)

            kind = path.parts[-1]
            if kind == "new":
                print(f"Opening {raw_path}")

                dataframes.append(pandas.read_csv(raw_path))
    
    dataframe = pandas.concat(dataframes, ignore_index=True)

    return dataframe

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('target_dir', help="the cargo's target directory")
    args = parser.parse_args()

    try:
        dataframe = read_criterion_raws(args.target_dir)            
    except OSError as err:
        print(err)
        return exit(1)
    
    time_benchmarks = ['huffman::pack', 'huffman::unpack']
    other_benchmarks = ['compression ratio']

    def filter_time_benchmarks(str) -> bool:
        return str in time_benchmarks
    
    def filter_other_benchmarks(str) -> bool:
        return str in other_benchmarks

    time_benchmarks_df = dataframe.loc[dataframe['group'].apply(filter_time_benchmarks)]
    time_benchmarks_df = time_benchmarks_df[['group', 'function', 'value', 'sample_measured_value', 'iteration_count', 'unit']] \
        .drop_duplicates(subset=['group', 'function', 'value'], keep='last', ignore_index=True)

    time_benchmarks_df['time_per_iteration_mean'] = time_benchmarks_df['sample_measured_value'] / time_benchmarks_df['iteration_count']
    # time_benchmarks_df.drop(axis=1, labels=['sample_measured_value', 'iteration_count'], inplace=True)

    other_benchmarks_df = dataframe.loc[dataframe['group'].apply(filter_other_benchmarks)]
    # other_benchmarks_df.loc[:, 'mean'] = other_benchmarks_df.groupby(['group', 'function', 'value']) ['sample_measured_value'].transform(lambda x: x.mean())
    # other_benchmarks_df = other_benchmarks_df[['group', 'function', 'value', 'mean', 'std']] \
    #     .drop_duplicates(subset=['group', 'function', 'value'], ignore_index=True)

    time_benchmarks_df.set_index('group', inplace=True)

    parents = pathlib.Path(sys.argv[0]).parents
    result_dir = parents[1].absolute() # skip the src directory
    result_dir = result_dir / "results"

    print(f"\nSaving the figures to {result_dir}...")

    try:
        os.mkdir(result_dir)
    except FileExistsError:
        pass

    sns.set_theme(style="ticks")

    plot = sns.relplot(time_benchmarks_df, x='value', y='time_per_iteration_mean', hue='function', col='group', kind='line', )
    plt.yscale('log')
    plot.set(xlabel='Input size [bytes]', ylabel='Time of the operation [ns]')
    plot.set_titles('{col_name}')

    new_name = {
        'huffman::pack': 'Compression',
        'huffman::unpack': 'Decompression'
    }

    for row in plot.axes:
        for ax in row:
            ax.set(title=new_name[ax.title._text])
    
    plot.savefig(result_dir / "performance.svg")

    plot = sns.catplot(other_benchmarks_df, x='value', y='sample_measured_value', hue='function', kind='bar', errorbar=None, aspect=2)
    plt.yscale('log')
    plt.ylabel('Compression ratio [input size / output size]')
    plt.xlabel('Amount of entropy in the input source')

    plot.savefig(result_dir / "compression_ratio.svg")

    print("Figures saved.")

if __name__ == "__main__":
    main()
