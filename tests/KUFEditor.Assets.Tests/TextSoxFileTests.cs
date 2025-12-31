using System.Text;
using Xunit;
using KUFEditor.Assets.TextSox;

namespace KUFEditor.Assets.Tests;

// Register codepages for Windows-1252 encoding used by TextSoxFile.
static class TextSoxFileTestsSetup
{
    static TextSoxFileTestsSetup()
    {
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
    }

    public static void EnsureRegistered()
    {
        // Just ensures the static constructor runs.
    }
}

public class TextSoxFileTests : IDisposable
{
    private readonly string _testDir;

    public TextSoxFileTests()
    {
        TextSoxFileTestsSetup.EnsureRegistered();
        _testDir = Path.Combine(Path.GetTempPath(), $"TextSoxTests_{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDir);
    }

    public void Dispose()
    {
        if (Directory.Exists(_testDir))
        {
            Directory.Delete(_testDir, recursive: true);
        }
    }

    [Fact]
    public void Load_SetsFilePath()
    {
        var filePath = CreateTestTextSoxFile(new[] { "Hello" });

        var data = TextSoxFile.Load(filePath);

        Assert.Equal(filePath, data.FilePath);
    }

    [Fact]
    public void Load_SetsFileName()
    {
        var filePath = CreateTestTextSoxFile(new[] { "Hello" });

        var data = TextSoxFile.Load(filePath);

        Assert.Equal(Path.GetFileName(filePath), data.FileName);
    }

    [Fact]
    public void Load_ParsesSingleEntry()
    {
        var filePath = CreateTestTextSoxFile(new[] { "Hello" });

        var data = TextSoxFile.Load(filePath);

        Assert.Single(data.Entries);
        Assert.Equal("Hello", data.Entries[0].Text);
    }

    [Fact]
    public void Load_ParsesMultipleEntries()
    {
        var filePath = CreateTestTextSoxFile(new[] { "First", "Second", "Third" });

        var data = TextSoxFile.Load(filePath);

        Assert.Equal(3, data.Entries.Count);
        Assert.Equal("First", data.Entries[0].Text);
        Assert.Equal("Second", data.Entries[1].Text);
        Assert.Equal("Third", data.Entries[2].Text);
    }

    [Fact]
    public void Load_SetsCorrectOffsets()
    {
        var filePath = CreateTestTextSoxFile(new[] { "Hello", "World" }, maxLength: 10);

        var data = TextSoxFile.Load(filePath);

        Assert.Equal(0, data.Entries[0].Offset);
        Assert.Equal(11, data.Entries[1].Offset); // 1 byte length + 10 bytes text
    }

    [Fact]
    public void Load_SetsMaxLength()
    {
        var filePath = CreateTestTextSoxFile(new[] { "Test" }, maxLength: 15);

        var data = TextSoxFile.Load(filePath);

        Assert.Equal(15, data.Entries[0].MaxLength);
    }

    [Fact]
    public void Load_PreservesRawData()
    {
        var filePath = CreateTestTextSoxFile(new[] { "Test" }, maxLength: 10);

        var data = TextSoxFile.Load(filePath);

        Assert.NotEmpty(data.RawData);
        Assert.Equal(11, data.RawData.Length); // 1 byte length + 10 bytes text
    }

    [Fact]
    public void Save_WritesFile()
    {
        var data = CreateTestTextSoxData(new[] { "Hello", "World" });
        var filePath = Path.Combine(_testDir, "output.sox");

        TextSoxFile.Save(data, filePath);

        Assert.True(File.Exists(filePath));
    }

    [Fact]
    public void RoundTrip_PreservesText()
    {
        var original = CreateTestTextSoxData(new[] { "First", "Second", "Third" });
        var filePath = Path.Combine(_testDir, "roundtrip.sox");

        TextSoxFile.Save(original, filePath);
        var loaded = TextSoxFile.Load(filePath);

        Assert.Equal(original.Entries.Count, loaded.Entries.Count);
        for (int i = 0; i < original.Entries.Count; i++)
        {
            Assert.Equal(original.Entries[i].Text, loaded.Entries[i].Text);
        }
    }

    [Fact]
    public void RoundTrip_PreservesMaxLength()
    {
        var original = CreateTestTextSoxData(new[] { "Test" }, maxLength: 20);
        var filePath = Path.Combine(_testDir, "roundtrip.sox");

        TextSoxFile.Save(original, filePath);
        var loaded = TextSoxFile.Load(filePath);

        Assert.Equal(20, loaded.Entries[0].MaxLength);
    }

    [Fact]
    public void Save_TruncatesOverflowText()
    {
        var data = CreateTestTextSoxData(new[] { "VeryLongTextThatExceedsLimit" }, maxLength: 10);
        var filePath = Path.Combine(_testDir, "truncated.sox");

        TextSoxFile.Save(data, filePath);
        var loaded = TextSoxFile.Load(filePath);

        Assert.Equal("VeryLongTe", loaded.Entries[0].Text);
    }

    [Fact]
    public void Save_PadsShortText()
    {
        var data = CreateTestTextSoxData(new[] { "Hi" }, maxLength: 10);
        var filePath = Path.Combine(_testDir, "padded.sox");

        TextSoxFile.Save(data, filePath);

        var bytes = File.ReadAllBytes(filePath);
        Assert.Equal(11, bytes.Length); // 1 byte length + 10 bytes (2 text + 8 padding)
        Assert.Equal(0, bytes[4]); // padding starts after "Hi"
    }

    [Fact]
    public void TextEntry_OffsetHex_FormatsCorrectly()
    {
        var entry = new TextEntry { Offset = 256 };

        Assert.Equal("0x0100", entry.OffsetHex);
    }

    [Fact]
    public void TextEntry_CurrentLength_ReturnsTextLength()
    {
        var entry = new TextEntry { Text = "Hello" };

        Assert.Equal(5, entry.CurrentLength);
    }

    [Fact]
    public void TextEntry_IsOverflow_ReturnsTrueWhenExceedsMax()
    {
        var entry = new TextEntry
        {
            Text = "VeryLongText",
            MaxLength = 5
        };

        Assert.True(entry.IsOverflow);
    }

    [Fact]
    public void TextEntry_IsOverflow_ReturnsFalseWhenWithinMax()
    {
        var entry = new TextEntry
        {
            Text = "Hi",
            MaxLength = 10
        };

        Assert.False(entry.IsOverflow);
    }

    [Fact]
    public void TextEntry_Remaining_ReturnsCorrectValue()
    {
        var entry = new TextEntry
        {
            Text = "Hello",
            MaxLength = 10
        };

        Assert.Equal(5, entry.Remaining);
    }

    [Fact]
    public void GetHexDump_FormatsCorrectly()
    {
        var data = new byte[] { 0x48, 0x65, 0x6C, 0x6C, 0x6F }; // "Hello"

        var hex = TextSoxFile.GetHexDump(data);

        Assert.Contains("00000000", hex);
        Assert.Contains("48 65 6C 6C 6F", hex);
        Assert.Contains("Hello", hex);
    }

    [Fact]
    public void GetHexDump_TruncatesLargeData()
    {
        var data = new byte[5000];

        var hex = TextSoxFile.GetHexDump(data, maxBytes: 100);

        Assert.Contains("more bytes", hex);
    }

    [Fact]
    public void GetHexDump_ReplacesNonPrintableChars()
    {
        var data = new byte[] { 0x00, 0x01, 0x02, 0x03 };

        var hex = TextSoxFile.GetHexDump(data);

        Assert.Contains("....", hex);
    }

    // ===== Helper Methods =====

    private string CreateTestTextSoxFile(string[] texts, byte maxLength = 11)
    {
        var filePath = Path.Combine(_testDir, $"test_{Guid.NewGuid():N}.sox");
        var encoding = Encoding.GetEncoding(1252);

        using var fs = File.Create(filePath);

        foreach (var text in texts)
        {
            // Write length prefix
            fs.WriteByte(maxLength);

            // Write text bytes with null padding
            var textBytes = new byte[maxLength];
            var sourceBytes = encoding.GetBytes(text);
            var copyLen = Math.Min(sourceBytes.Length, maxLength);
            Array.Copy(sourceBytes, textBytes, copyLen);
            fs.Write(textBytes, 0, maxLength);
        }

        return filePath;
    }

    private static TextSoxData CreateTestTextSoxData(string[] texts, byte maxLength = 11)
    {
        var data = new TextSoxData
        {
            FilePath = "/test/file.sox",
            FileName = "file.sox"
        };

        int offset = 0;
        for (int i = 0; i < texts.Length; i++)
        {
            data.Entries.Add(new TextEntry
            {
                Index = i,
                Offset = offset,
                MaxLength = maxLength,
                Text = texts[i]
            });
            offset += 1 + maxLength;
        }

        return data;
    }
}
