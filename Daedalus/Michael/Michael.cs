using System.Diagnostics;

namespace Michael;

public class Michael
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
            _ => FoundSolution()
        };
    }

    private int FoundSolution()
    {
        _stopwatch.Stop();
        var s =
            $"Solution found, flag message: {"connection.get().getElementsByTag(\"p\").get(0).text()"}\nPath length: {"path.length()"}\nExecution time: {_stopwatch.Elapsed}\nPath to solution: {"add Path to solution later"}";
        Console.WriteLine(s);
        Environment.Exit(0);
        return -1;
    }
}