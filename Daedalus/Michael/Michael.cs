using System.Diagnostics;
using System.Text.RegularExpressions;

namespace Michael;

public partial class Michael
{
    private const int MaxPathLength = 2499;
    private readonly HttpClient _httpClient;
    private readonly Stopwatch _stopwatch;
    private HttpResponseMessage _responseMessage;

    public Michael()
    {
        // _httpClient = new HttpClient { BaseAddress = new Uri("https://daedalus.defi.info.cegepmontpetit.ca/move") };
        _httpClient = new HttpClient { BaseAddress = new Uri("http://127.0.0.1:8173/move") };
        _stopwatch = Stopwatch.StartNew();

        _responseMessage = MichaelUtils.MakeRequestWithCookie(_httpClient, "");
        unsafe
        {
            var pathBuffer = stackalloc char[MaxPathLength];
            RunMichael(pathBuffer, 0);
        }
    }

    private unsafe void RunMichael(char* pathBuffer, int currentDepth)
    {
        for (var i = 0; i < 4; i++)
        {
            if (currentDepth > 0 && pathBuffer[currentDepth - 1] == MichaelUtils.OppositeDirections[i]) continue;

            var moveDirection = MichaelUtils.MovePriority[i];
            pathBuffer[currentDepth] = moveDirection;
            var newDepth = currentDepth + 1;

            if (Move(pathBuffer, newDepth) != 1) continue;

            pathBuffer[currentDepth + 1] = moveDirection;
            RunMichael(pathBuffer, newDepth + 1);
        }
    }

    private unsafe int Move(char* pathBuffer, int length)
    {
        var path = string.Create(length, ((IntPtr)pathBuffer, length), static (span, state) =>
        {
            var (ptr, len) = state;
            new ReadOnlySpan<char>((char*)ptr, len).CopyTo(span);
        });
        _responseMessage = MichaelUtils.MakeRequestWithCookie(_httpClient, path);

        var messageLength = MichaelUtils.GetContentLength(_responseMessage);
        return messageLength switch
        {
            1730 => 0,
            1744 => 1,
            1759 => FoundSolution(path),
            1774 => 1,
            _ => -1
        };
    }

    private int FoundSolution(string path)
    {
        _stopwatch.Stop();

        var responseBody = _responseMessage.Content.ReadAsStringAsync().GetAwaiter().GetResult();
        var match = FlagRegex().Match(responseBody);
        var flagMessage = match.Success ? match.Value : "There's no flag in the HTML";

        var s =
            $"Solution found, flag message: {(string.IsNullOrEmpty(flagMessage) ? responseBody : flagMessage)}\n" +
            $"Path length: {path.Length}\n" +
            $"Execution time: {_stopwatch.Elapsed}\n" +
            $"Path to solution: {path}";
        Console.WriteLine(s);
        return -1;
    }

    [GeneratedRegex(@"CEM\{.*?\}")]
    private static partial Regex FlagRegex();
}