// See https://aka.ms/new-console-template for more information

using BenchmarkDotNet.Reports;
using BenchmarkDotNet.Running;

namespace Benchmarks;

internal static class Program
{
    public static void Main(string[] args)
    {
        Summary summary = BenchmarkRunner.Run<Benchmark>();
    }
}