using BenchmarkDotNet.Configs;
using BenchmarkDotNet.Exporters.Csv;
using BenchmarkDotNet.Running;

namespace Benchmarks;

internal static class Program
{
    public static void Main()
    {
        var config = ManualConfig.CreateMinimumViable();

        BenchmarkRunner.Run<Benchmark>(config);
    }
}