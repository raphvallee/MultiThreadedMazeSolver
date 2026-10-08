using System.Text;

namespace Michael;

public static class MichaelUtils
{
    public static readonly char[] MovePriority = ['r', 'd', 'l', 'u'];
    public static readonly char[] OppositeDirections = ['l', 'u', 'r', 'd'];

    private static string ToBase64(string content)
    {
        var plainTextBytes = Encoding.UTF8.GetBytes(content);
        return Convert.ToBase64String(plainTextBytes);
    }

    public static HttpResponseMessage MakeRequestWithCookie(HttpClient httpClient, string value)
    {
        // TODO find faster way to set cookie
        httpClient.DefaultRequestHeaders.Clear();
        httpClient.DefaultRequestHeaders.TryAddWithoutValidation("Cookie", "path=" + ToBase64(value));
        return MakeRequest(httpClient);
    }

    private static HttpResponseMessage MakeRequest(HttpClient httpClient)
    {
        return httpClient.GetAsync("").Result;
    }

    public static long GetContentLength(HttpResponseMessage responseMessage)
    {
        return responseMessage.Content.Headers.ContentLength.GetValueOrDefault();
    }
}
