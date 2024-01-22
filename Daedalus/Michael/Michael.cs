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

        _responseMessage = MichaelUtils.SetPathCookie(_httpClient, "").Result;
        RunMichael("");
    }

    private Task RunMichael(string path)
    {
        var tasks = Enumerable.Range(0, 4)
            .Select(async i =>
            {
                var s = MichaelUtils.MovePriority[i];
                // var moveTask = Move(path + s);
                if (!path.EndsWith(MichaelUtils.OppositeDirections[i]) && await Move(path + s) == 1)
                    //System.out.println("Creating new thread with path: " + path + s + s);
                    await RunMichael(path + s + s);
            }).ToArray();
        Task.WaitAll(tasks);

        // Parallel.ForEachAsync(Enumerable.Range(0, 3), i =>
        // {
        //     var s = MichaelUtils.MovePriority[i];
        //     // var moveTask = Move(path + s);
        //     if (!path.EndsWith(MichaelUtils.OppositeDirections[i]) && (await Move(path + s)) == 1)
        //     {
        //         //System.out.println("Creating new thread with path: " + path + s + s);
        //         RunMichael(path + s + s);
        //     }
        //
        //     return ValueTask.CompletedTask;
        // });
        return Task.CompletedTask;
    }

    private async Task<int> Move(string path)
    {
        try
        {
            if (_httpClient == null) Console.WriteLine("_httpClient == null");
            if (_responseMessage == null) Console.WriteLine("_responseMessage == null");
            _responseMessage = await MichaelUtils.SetPathCookie(_httpClient, path);

            // parses the message element
            var messageLength = MichaelUtils.GetContentLength(_responseMessage);
            Console.WriteLine(path);
            // return number based on message
            return messageLength switch
            {
                1786 => 0,
                1800 => 1,
                5379 => Move(path).Result,
                _ => FoundSolution()
            };
        }
        catch (IOException exception)
        {
            Console.WriteLine(exception.Message);
            return -1;
        }
    }

    private int FoundSolution()
    {
        _stopwatch.Stop();
        var s =
            $"Solution found, flag message: {"connection.get().getElementsByTag(\"p\").get(0).text()"}\nPath length: {"path.length()"}\n Execution time: {_stopwatch.Elapsed}\nPath to solution: {"add Path to solution later"}";
        Console.WriteLine(s);
        Environment.Exit(0);
        return -1;
    }
}