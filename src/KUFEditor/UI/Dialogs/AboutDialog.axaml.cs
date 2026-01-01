using System.Reflection;
using Avalonia.Controls;
using Avalonia.Interactivity;

namespace KUFEditor.UI.Dialogs;

public partial class AboutDialog : Window
{
    public AboutDialog()
    {
        InitializeComponent();
        SetupControls();
    }

    private void SetupControls()
    {
        var version = Assembly.GetExecutingAssembly().GetName().Version;
        var versionText = this.FindControl<TextBlock>("VersionText");
        if (versionText != null && version != null)
        {
            versionText.Text = $"Version {version.Major}.{version.Minor}.{version.Build}";
        }

        var closeButton = this.FindControl<Button>("CloseButton");
        var okButton = this.FindControl<Button>("OkButton");
        var gitHubButton = this.FindControl<Button>("GitHubButton");

        if (closeButton != null) closeButton.Click += OnClose;
        if (okButton != null) okButton.Click += OnClose;
        if (gitHubButton != null) gitHubButton.Click += OnGitHub;
    }

    private void OnClose(object? sender, RoutedEventArgs e)
    {
        Close();
    }

    private async void OnGitHub(object? sender, RoutedEventArgs e)
    {
        var launcher = TopLevel.GetTopLevel(this)?.Launcher;
        if (launcher != null)
        {
            await launcher.LaunchUriAsync(new System.Uri("https://github.com/rdeusser/kufeditor"));
        }
    }
}
