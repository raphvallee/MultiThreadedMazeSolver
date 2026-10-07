using System.Diagnostics;
using System.Text;
using System.Text.RegularExpressions;

namespace Michael;

public partial class Michael
{
    private readonly HttpClient _httpClient;
    private readonly Stopwatch _stopwatch;
    private readonly StringBuilder _stringBuilder;
    private HttpResponseMessage _responseMessage;

    public Michael()
    {
        _stringBuilder = new StringBuilder();
        _httpClient = new HttpClient { BaseAddress = new Uri("http://127.0.0.1:8173/move") };
        _stopwatch = Stopwatch.StartNew();

        _responseMessage = MichaelUtils.MakeRequestWithCookie(_httpClient, "");
        RunMichael("");
    }

    private void RunMichael(string path)
    {
        for (var i = 0; i < 4; i++)
            if (!MichaelUtils.EndsWithChar(path, MichaelUtils.OppositeDirections[i]))
            {
                var moveDirection = MichaelUtils.MovePriority[i];

                var currentMove = path + moveDirection + moveDirection;
                if (Move(currentMove) == 1) RunMichael(currentMove);
            }
    }

    private int Move(string path)
    {
        _responseMessage = MichaelUtils.MakeRequestWithCookie(_httpClient, path);

        var messageLength = MichaelUtils.GetContentLength(_responseMessage);

        return messageLength switch
        {
            1730 => 0,
            1744 => 1,
            _ => FoundSolution(path)
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
        Environment.Exit(0);
        return -1;
    }

    [GeneratedRegex(@"CEM\{.*?\}")]
    private static partial Regex FlagRegex();
}