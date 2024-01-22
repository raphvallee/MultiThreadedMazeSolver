using System.Text;

namespace Michael;

public static class Utils
{
    public static readonly string[] MovePriority = ["r", "d", "l", "u"];
    public static readonly string[] OppositeDirections = ["l", "u", "r", "d"];

    public static string ToBase64(string content)
    {
        var plainTextBytes = Encoding.UTF8.GetBytes(content);
        return Convert.ToBase64String(plainTextBytes);
    }
}