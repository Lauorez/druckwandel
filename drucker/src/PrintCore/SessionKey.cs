using System.Security.Cryptography;
using System.Text;

namespace ERechnung.PrintCore;

public static class SessionKey
{
    public static string From(string sessionId)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(sessionId);
        byte[] digest = SHA256.HashData(Encoding.UTF8.GetBytes(sessionId));
        return Convert.ToHexStringLower(digest);
    }
}
