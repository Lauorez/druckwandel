using System.Text.Json.Serialization;

namespace ERechnung.PrintCore;

[JsonSerializable(typeof(PrintJobRecord))]
[JsonSerializable(typeof(PrintJobEvent))]
[JsonSerializable(typeof(ReviewHandoffRecord))]
internal partial class PrintJsonContext : JsonSerializerContext;
