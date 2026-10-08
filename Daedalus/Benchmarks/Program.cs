using BenchmarkDotNet.Columns;
using BenchmarkDotNet.Configs;
using BenchmarkDotNet.Running;

namespace Benchmarks;

public class Program
{
    public static void Main(string[] args)
    {
        var config = DefaultConfig.Instance.AddColumnProvider(DefaultColumnProviders.Instance);
        var summaries = BenchmarkSwitcher.FromAssembly(typeof(Program).Assembly).Run(args, config);
    }
}