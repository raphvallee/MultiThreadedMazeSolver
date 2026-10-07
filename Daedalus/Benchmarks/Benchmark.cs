using System.Buffers.Text;
using System.Security.Cryptography;
using System.Text;
using BenchmarkDotNet.Attributes;
using BenchmarkDotNet.Configs;
using BenchmarkDotNet.Jobs;

namespace Benchmarks;

[SimpleJob(RuntimeMoniker.NativeAot10_0)]
[GroupBenchmarksBy(BenchmarkLogicalGroupRule.ByParams)]
public class Benchmark
{
    // [Params(100, 500, 1000, 1624, 2000)] public int TextLength;
    [Params(1624)] public int TextLength;

    private string _inputString = null!;

    [GlobalSetup]
    public void Setup()
    {
        // Generate random string matching specified length
        var random = new Random(42); // Fixed seed for reproducible benchmarks
        const string chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 ";

        _inputString = string.Create(TextLength, random, (span, rng) =>
        {
            for (var i = 0; i < span.Length; i++)
            {
                span[i] = chars[rng.Next(chars.Length)];
            }
        });
    }

    /// <summary>
    /// Standard approach: Converts text to byte[] then calls Convert.ToBase64String.
    /// 350ns
    /// </summary>
    [Benchmark(Baseline = true)]
    public string Standard_ConvertToBase64String()
    {
        var bytes = Encoding.UTF8.GetBytes(_inputString);
        return Convert.ToBase64String(bytes);
    }

    /// <summary>
    /// Built-in URL-safe Base64 encoding using System.Buffers.Text.Base64Url.
    /// 325ns
    /// </summary>
    [Benchmark]
    public string UrlSafe_Base64UrlEncodeToString()
    {
        var bytes = Encoding.UTF8.GetBytes(_inputString);
        return Base64Url.EncodeToString(bytes);
    }
}