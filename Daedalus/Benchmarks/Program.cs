// See https://aka.ms/new-console-template for more information

using BenchmarkDotNet.Running;

namespace Benchmarks;

internal static class Program
{
    public static void Main(string[] args)
    {
        var summary = BenchmarkRunner.Run<Benchmark>();
    }
}