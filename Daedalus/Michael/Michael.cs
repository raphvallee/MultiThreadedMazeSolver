using System.Diagnostics;
using System.Text.RegularExpressions;

namespace Michael;

public partial class Michael
{
    private readonly HttpClient _httpClient;
    private readonly Stopwatch _stopwatch;
    private HttpResponseMessage _responseMessage;

    public Michael()
    {
        _httpClient = new HttpClient { BaseAddress = new Uri("https://daedalus.defi.info.cegepmontpetit.ca/move") };
        _stopwatch = Stopwatch.StartNew();

        _responseMessage = MichaelUtils.SetPathCookie(_httpClient, "");
        RunMichael("");
    }

    private void RunMichael(string path)
    {
        for (var i = 0; i < 4; i++)
            if (!path.EndsWith(MichaelUtils.OppositeDirections[i]))
            {
                var moveDirection = MichaelUtils.MovePriority[i];
                var currentMove = path + moveDirection;
                if (Move(currentMove) == 1) RunMichael(currentMove + moveDirection);
            }
    }

    private int Move(string path)
    {
        _responseMessage = MichaelUtils.SetPathCookie(_httpClient, path);

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
        var flagMessage = match.Success ? match.Value : "Couldnt find the flag with regex";

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