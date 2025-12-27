using System;
using System.Collections.ObjectModel;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Threading;

namespace KUFEditor.UI.Views;

public partial class OutputPanel : UserControl
{
    public ObservableCollection<ErrorItem> Errors { get; }
    private readonly ObservableCollection<string> commandHistory = new();
    private int historyIndex = -1;

    public OutputPanel()
    {
        InitializeComponent();
        Errors = new ObservableCollection<ErrorItem>();

        var errorGrid = this.FindControl<DataGrid>("ErrorGrid");
        if (errorGrid != null)
            errorGrid.ItemsSource = Errors;
    }

    public void WriteLine(string text)
    {
        var outputText = this.FindControl<TextBox>("OutputText");
        if (outputText != null)
        {
            outputText.Text += text + Environment.NewLine;
            AutoScroll();
        }
    }

    public void WriteError(string message, string file = "", int line = 0)
    {
        Errors.Add(new ErrorItem
        {
            Type = "Error",
            Message = message,
            File = file,
            Line = line
        });

        UpdateErrorCount();
        WriteLine($"[ERROR] {message}");
    }

    public void WriteWarning(string message, string file = "", int line = 0)
    {
        Errors.Add(new ErrorItem
        {
            Type = "Warning",
            Message = message,
            File = file,
            Line = line
        });

        UpdateErrorCount();
        WriteLine($"[WARNING] {message}");
    }

    public void WriteConsole(string text)
    {
        var consoleOutput = this.FindControl<TextBox>("ConsoleOutput");
        if (consoleOutput != null)
        {
            consoleOutput.Text += text + Environment.NewLine;

            var scroller = this.FindControl<ScrollViewer>("ConsoleScroller");
            scroller?.ScrollToEnd();
        }
    }

    private void OnClearOutput(object? sender, RoutedEventArgs e)
    {
        var outputText = this.FindControl<TextBox>("OutputText");
        if (outputText != null)
            outputText.Text = string.Empty;
    }

    private void OnClearErrors(object? sender, RoutedEventArgs e)
    {
        Errors.Clear();
        UpdateErrorCount();
    }

    private void OnSendCommand(object? sender, RoutedEventArgs e)
    {
        ProcessCommand();
    }

    private void OnCommandKeyDown(object? sender, KeyEventArgs e)
    {
        var input = sender as TextBox;
        if (input == null) return;

        switch (e.Key)
        {
            case Key.Enter:
                ProcessCommand();
                e.Handled = true;
                break;

            case Key.Up:
                if (historyIndex < commandHistory.Count - 1)
                {
                    historyIndex++;
                    input.Text = commandHistory[commandHistory.Count - 1 - historyIndex];
                }
                e.Handled = true;
                break;

            case Key.Down:
                if (historyIndex > 0)
                {
                    historyIndex--;
                    input.Text = commandHistory[commandHistory.Count - 1 - historyIndex];
                }
                else if (historyIndex == 0)
                {
                    historyIndex = -1;
                    input.Text = string.Empty;
                }
                e.Handled = true;
                break;
        }
    }

    private void ProcessCommand()
    {
        var input = this.FindControl<TextBox>("CommandInput");
        if (input == null || string.IsNullOrWhiteSpace(input.Text))
            return;

        var command = input.Text;
        commandHistory.Add(command);
        historyIndex = -1;

        WriteConsole($"> {command}");

        // process command
        ExecuteCommand(command);

        input.Text = string.Empty;
    }

    private void ExecuteCommand(string command)
    {
        var parts = command.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        if (parts.Length == 0) return;

        switch (parts[0].ToLower())
        {
            case "help":
                WriteConsole("Available commands:");
                WriteConsole("  help - Show this help");
                WriteConsole("  clear - Clear console");
                WriteConsole("  load <file> - Load a file");
                WriteConsole("  save - Save current file");
                WriteConsole("  exit - Exit application");
                break;

            case "clear":
                var consoleOutput = this.FindControl<TextBox>("ConsoleOutput");
                if (consoleOutput != null)
                    consoleOutput.Text = string.Empty;
                break;

            default:
                WriteConsole($"Unknown command: {parts[0]}");
                break;
        }
    }

    private void UpdateErrorCount()
    {
        var errorCount = this.FindControl<TextBlock>("ErrorCount");
        if (errorCount != null)
        {
            var count = Errors.Count;
            errorCount.Text = $"{count} Error{(count != 1 ? "s" : "")}";
        }
    }

    private void AutoScroll()
    {
        var autoScroll = this.FindControl<CheckBox>("AutoScrollCheck");
        if (autoScroll?.IsChecked == true)
        {
            var scroller = this.FindControl<ScrollViewer>("OutputScroller");
            Dispatcher.UIThread.Post(() => scroller?.ScrollToEnd());
        }
    }
}

public class ErrorItem
{
    public string Type { get; set; } = string.Empty;
    public string Message { get; set; } = string.Empty;
    public string File { get; set; } = string.Empty;
    public int Line { get; set; }
}