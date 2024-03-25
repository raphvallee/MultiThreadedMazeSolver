using System.Diagnostics;

namespace Michael;

public class Michael
{
    private readonly HttpClient _httpClient;
    private readonly Stopwatch _stopwatch;
    private HttpResponseMessage _responseMessage;

    public Michael()
    {
        _httpClient = new HttpClient { BaseAddress = new Uri("https://ctf.ageei.org/daedalusv3105/move") };
        _stopwatch = Stopwatch.StartNew();

        _responseMessage = MichaelUtils.SetPathCookie(_httpClient, "");
        RunMichael("");
    }

    private void RunMichael(string path)
    {
        for (int i = 0; i < 4; i++) {
            string s = MichaelUtils.MovePriority[i];
            if (!path.EndsWith(MichaelUtils.OppositeDirections[i]) && Move(path + s) == 1) {
                RunMichael(path + s + s);
            }
        }
    }

    private int Move(string path)
    {
        Console.WriteLine(path);
            
        _responseMessage = MichaelUtils.SetPathCookie(_httpClient, path);
            
        var messageLength = MichaelUtils.GetContentLength(_responseMessage);
            
        return messageLength switch
        {
            1786 => 0,
            1800 => 1,
            5379 => Move(path),
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