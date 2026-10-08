using BenchmarkDotNet.Attributes;
using BenchmarkDotNet.Configs;
using BenchmarkDotNet.Jobs;

namespace Benchmarks;

[SimpleJob(RuntimeMoniker.NativeAot10_0)]
[GroupBenchmarksBy(BenchmarkLogicalGroupRule.ByParams)]
public class Benchmark
{
    [Benchmark]
    public void RunMichael()
    {
        var michael = new Michael.Michael();
    }
}