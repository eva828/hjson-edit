using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Text;
using Hjson;

static class Program
{
    static readonly PropertyInfo? PrimitiveValue =
        typeof(JsonValue).Assembly.GetType("Hjson.JsonPrimitive")
            ?.GetProperty("Value", BindingFlags.Instance | BindingFlags.NonPublic);

    static int Main(string[] args)
    {
        if (args.Length < 2) { Console.Error.WriteLine("usage: Oracle <cases|assets|dir> <path>"); return 2; }
        switch (args[0])
        {
            case "cases": RunCases(args[1]); break;
            case "assets": RunAssets(args[1]); break;
            case "dir": RunDir(args[1]); break;
            default: Console.Error.WriteLine($"unknown mode: {args[0]}"); return 2;
        }
        return 0;
    }

    static void RunCases(string path)
    {
        foreach (var line in File.ReadAllLines(path))
        {
            if (line.Length == 0 || line.StartsWith("#")) continue;
            var parts = line.Split('\t');
            var label = parts[0];
            var input = Unescape(parts[1]);
            Console.WriteLine(label + "\t" + Parse(input));
        }
    }

    // ディレクトリ配下の .hjson を再帰的に処理する（相対パスを id にする）
    static void RunDir(string dir)
    {
        var files = new List<string>(Directory.GetFiles(dir, "*.hjson", SearchOption.AllDirectories));
        files.Sort(StringComparer.Ordinal);
        var root = Path.GetFullPath(dir).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
        foreach (var file in files)
        {
            var full = Path.GetFullPath(file);
            var name = full.StartsWith(root, StringComparison.Ordinal) ? full.Substring(root.Length) : full;
            Console.WriteLine(name + "\t" + Parse(File.ReadAllText(file)));
        }
    }

    static void RunAssets(string dir)
    {
        var files = new List<string>(Directory.GetFiles(dir));
        files.Sort(StringComparer.Ordinal);
        foreach (var file in files)
        {
            var name = Path.GetFileName(file);
            if (!name.EndsWith("_test.hjson") && !name.EndsWith("_test.json")) continue;
            Console.WriteLine(name + "\t" + Parse(File.ReadAllText(file)));
        }
    }

    static string Parse(string input)
    {
        try
        {
            var value = HjsonValue.Parse(input);
            return "OK\t" + Canon(value);
        }
        catch (Exception e)
        {
            var msg = (e.Message ?? e.GetType().Name).Replace("\r", "").Split('\n')[0];
            return "ERR\t" + msg;
        }
    }

    static string Canon(JsonValue? v)
    {
        if (v is null) return "null";
        switch (v.JsonType)
        {
            case JsonType.Object:
                {
                    var sb = new StringBuilder("{");
                    var first = true;
                    foreach (var pair in (IDictionary<string, JsonValue>)v)
                    {
                        if (!first) sb.Append(',');
                        first = false;
                        sb.Append('"').Append(Escape(pair.Key)).Append("\":").Append(Canon(pair.Value));
                    }
                    return sb.Append('}').ToString();
                }
            case JsonType.Array:
                {
                    var sb = new StringBuilder("[");
                    var first = true;
                    foreach (var item in (IList<JsonValue>)v)
                    {
                        if (!first) sb.Append(',');
                        first = false;
                        sb.Append(Canon(item));
                    }
                    return sb.Append(']').ToString();
                }
            case JsonType.Boolean:
                return (bool)v ? "true" : "false";
            case JsonType.String:
                return "\"" + Escape((string)v) + "\"";
            case JsonType.Number:
                {
                    var raw = PrimitiveValue?.GetValue(v);
                    return raw switch
                    {
                        long l => l.ToString(CultureInfo.InvariantCulture),
                        double d => FormatDouble(d),
                        _ => "?" + (raw?.GetType().Name ?? "null"),
                    };
                }
            default:
                return "null";
        }
    }

    static string FormatDouble(double d)
    {
        if (double.IsNaN(d)) return "NaN";
        if (double.IsPositiveInfinity(d)) return "inf";
        if (double.IsNegativeInfinity(d)) return "-inf";
        var s = d.ToString("R", CultureInfo.InvariantCulture);
        if (s.Contains('E'))
        {
            s = s.Replace("E", "e");
            var m = s.IndexOf('e');
            var mant = s.Substring(0, m);
            var exp = s.Substring(m + 1);
            if (exp.StartsWith("+")) exp = exp.Substring(1);
            var neg = exp.StartsWith("-");
            if (neg) exp = exp.Substring(1);
            exp = exp.TrimStart('0');
            if (exp.Length == 0) exp = "0";
            s = mant + "e" + (neg ? "-" : "") + exp;
        }
        if (!s.Contains('.') && !s.Contains('e')) s += ".0";
        return s;
    }

    static string Escape(string s)
    {
        var sb = new StringBuilder(s.Length + 8);
        foreach (var c in s)
        {
            switch (c)
            {
                case '\\': sb.Append("\\\\"); break;
                case '"': sb.Append("\\\""); break;
                case '\n': sb.Append("\\n"); break;
                case '\r': sb.Append("\\r"); break;
                case '\t': sb.Append("\\t"); break;
                default: sb.Append(c); break;
            }
        }
        return sb.ToString();
    }

    static string Unescape(string s)
    {
        var sb = new StringBuilder(s.Length);
        for (var i = 0; i < s.Length; i++)
        {
            var c = s[i];
            if (c != '\\') { sb.Append(c); continue; }
            i++;
            switch (s[i])
            {
                case 'n': sb.Append('\n'); break;
                case 'r': sb.Append('\r'); break;
                case 't': sb.Append('\t'); break;
                case '\\': sb.Append('\\'); break;
                case 'u':
                    {
                        var hex = s.Substring(i + 1, 4);
                        sb.Append((char)Convert.ToInt32(hex, 16));
                        i += 4;
                        break;
                    }
                default: sb.Append('\\').Append(s[i]); break;
            }
        }
        return sb.ToString();
    }
}
