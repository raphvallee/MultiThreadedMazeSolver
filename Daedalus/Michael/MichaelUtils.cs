using System.Text;

namespace Michael;

public static class MichaelUtils
{
    public static readonly string[] MovePriority = ["r", "d", "l", "u"];
    public static readonly char[] OppositeDirections = ['l', 'u', 'r', 'd'];
    private const string EmptyString = "";

    private static string ToBase64(string content)
    {
        var plainTextBytes = Encoding.UTF8.GetBytes(content);
        return Convert.ToBase64String(plainTextBytes);
    }

    public static HttpResponseMessage MakeRequestWithCookie(HttpClient httpClient, string value)
    {
        httpClient.DefaultRequestHeaders.Clear();
        httpClient.DefaultRequestHeaders.Add("Cookie", "path=" + ToBase64(value));
        return MakeRequest(httpClient);
    }

    private static HttpResponseMessage MakeRequest(HttpClient httpClient)
    {
        return httpClient.GetAsync(EmptyString).Result;
    }

    public static long GetContentLength(HttpResponseMessage responseMessage)
    {
        return responseMessage.Content.Headers.ContentLength.GetValueOrDefault();
    }

    public static bool EndsWithChar(string current, char c)
    {
        if (current.Length == 0) return false;
        return current[^1] == c;
    }
}