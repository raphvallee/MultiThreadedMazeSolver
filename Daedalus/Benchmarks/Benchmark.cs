using BenchmarkDotNet.Attributes;

namespace Benchmarks;

public class Benchmark
{
    private readonly HttpResponseMessage _responseMessage;

    public Benchmark()
    {
        var h = new HttpClient { BaseAddress = new Uri("https://ctf.ageei.org/daedalusv3105/move") };
        _responseMessage = h.GetAsync("").Result;
    }

    [Benchmark]
    public long? HeaderDotContentLength() // 16ns
    {
        var s = _responseMessage.Content.Headers.ContentLength;
        return s;
    }

    // [Benchmark]
    // public long? ReadAsStringAsyncDotLength() // 545ns
    // {
    //     var s = _responseMessage.Content.ReadAsStringAsync().Result.Length;
    //     return s;
    // }
}