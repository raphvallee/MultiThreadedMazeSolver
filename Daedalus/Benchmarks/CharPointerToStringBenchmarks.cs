using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using BenchmarkDotNet.Attributes;

// ReSharper disable InvalidXmlDocComment

namespace Benchmarks;

[MemoryDiagnoser] // Measures memory allocations and GC collections
public unsafe class CharPointerToStringBenchmarks
{
    private GCHandle _gcHandle;
    private int _length;
    private char* _pathBuffer;

    [GlobalSetup]
    public void Setup()
    {
        // Sample path simulating unmanaged string data
        var sample = @"C:\Users\Public\Documents\Projects\BenchmarkSuite\TestFolder\File.txt";
        _length = sample.Length;

        // Pin array memory to get a stable unmanaged char pointer for benchmarks
        _pathBuffer = (char*)_gcHandle.AddrOfPinnedObject();
    }

    [GlobalCleanup]
    public void Cleanup()
    {
        if (_gcHandle.IsAllocated) _gcHandle.Free();
    }

    /// <summary>
    ///     Standard pointer string constructor.
    /// </summary>
    [Benchmark]
    public string Ctor_CharPointer()
    {
        return new string(_pathBuffer, 0, _length);
    }

    /// <summary>
    ///     Marshal helper method (System.Runtime.InteropServices).
    /// </summary>
    [Benchmark]
    public string Marshal_PtrToStringUni()
    {
        return Marshal.PtrToStringUni((IntPtr)_pathBuffer, _length)!;
    }

    /// <summary>
    ///     Baseline: High-performance string allocation API using Span CopyTo.
    /// </summary>
    [Benchmark]
    public string String_Create_SpanCopy()
    {
        return string.Create(_length, (IntPtr)_pathBuffer,
            static (span, ptr) => { new ReadOnlySpan<char>((char*)ptr, span.Length).CopyTo(span); });
    }

    /// <summary>
    ///     Standard pointer string constructor.
    /// </summary>
    [Benchmark(Baseline = true)]
    public string Ctor_CharPointer_StartIndex_Length()
    {
        return new string(_pathBuffer, 0, _length);
    }

    /// <summary>
    ///     Direct char* pointer constructor without start index offset.
    /// </summary>
    [Benchmark]
    public string Ctor_CharPointer_Length()
    {
        return new string(_pathBuffer, 0, _length); // Equivalent when offset is 0
    }

    /// <summary>
    ///     Span constructor via ReadOnlySpan
    ///     <char>
    ///         constructor.
    ///         Works with unsafe char pointer explicitly converted to ReadOnlySpan.
    /// </summary>
    [Benchmark]
    public string Ctor_ReadOnlySpan()
    {
        return new string(new ReadOnlySpan<char>(_pathBuffer, _length));
    }

    /// <summary>
    ///     Extension method on ReadOnlySpan<char> (idiomatic modern .NET).
    /// </summary>
    [Benchmark]
    public string Span_ToString()
    {
        return new ReadOnlySpan<char>(_pathBuffer, _length).ToString();
    }

    /// <summary>
    ///     High-performance string allocation API for unsafe pointer copying.
    /// </summary>
    [Benchmark]
    public string String_Create()
    {
        // Safe context pass-through avoiding closures
        return string.Create(_length, ((IntPtr)_pathBuffer, _length), static (span, state) =>
        {
            var (ptr, len) = state;
            new ReadOnlySpan<char>((char*)ptr, len).CopyTo(span);
        });
    }

    /// <summary>
    ///     string.Create using Unsafe.CopyBlockUnaligned for direct raw bytes transfer.
    /// </summary>
    [Benchmark]
    public string String_Create_UnsafeCopyBlock()
    {
        return string.Create(_length, (IntPtr)_pathBuffer, static (span, ptr) =>
        {
            fixed (char* destination = span)
            {
                Unsafe.CopyBlockUnaligned(
                    destination,
                    (void*)ptr,
                    (uint)(span.Length * sizeof(char))
                );
            }
        });
    }

    /// <summary>
    ///     string.Create using fast native Buffer.MemoryCopy intrinsics.
    /// </summary>
    [Benchmark]
    public string String_Create_BufferMemoryCopy()
    {
        return string.Create(_length, (IntPtr)_pathBuffer, static (span, ptr) =>
        {
            fixed (char* destination = span)
            {
                var bytesToCopy = (ulong)(span.Length * sizeof(char));
                Buffer.MemoryCopy((void*)ptr, destination, bytesToCopy, bytesToCopy);
            }
        });
    }

    /// <summary>
    ///     Fast conversion for null-terminated char pointers where length isn't passed explicitly.
    /// </summary>
    [Benchmark]
    public string NullTerminated_MemoryMarshal_ToString()
    {
        return MemoryMarshal.CreateReadOnlySpanFromNullTerminated(_pathBuffer).ToString();
    }

    /// <summary>
    ///     Encoding.Unicode.GetString overload taking ReadOnlySpan<char> / byte pointer wrapper.
    /// </summary>
    [Benchmark]
    public string Encoding_Unicode_GetString()
    {
        return Encoding.Unicode.GetString((byte*)_pathBuffer, _length * sizeof(char));
    }

    /// <summary>
    ///     Fast string allocation via System.Text.StringBuilder / Fast append.
    /// </summary>
    [Benchmark]
    public string StringBuilder_Append_ToString()
    {
        // Pre-sized builder avoids reallocations
        return new StringBuilder(_length)
            .Append(_pathBuffer, _length)
            .ToString();
    }
}