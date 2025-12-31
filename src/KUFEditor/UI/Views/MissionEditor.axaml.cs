using System;
using System.Collections.ObjectModel;
using System.IO;
using Avalonia.Controls;
using Avalonia.Interactivity;
using KUFEditor.Assets.Mission;

namespace KUFEditor.UI.Views;

public partial class MissionEditor : UserControl
{
    private MissionData? _mission;
    private string? _filePath;
    private TroopBlock? _selectedTroop;

    public MissionEditor()
    {
        InitializeComponent();
        SetupControls();
    }

    private void SetupControls()
    {
        var saveButton = this.FindControl<Button>("SaveButton");
        var reloadButton = this.FindControl<Button>("ReloadButton");
        var troopList = this.FindControl<ListBox>("TroopList");
        var categoryCombo = this.FindControl<ComboBox>("CategoryCombo");
        var allegianceCombo = this.FindControl<ComboBox>("AllegianceCombo");

        if (saveButton != null) saveButton.Click += OnSave;
        if (reloadButton != null) reloadButton.Click += OnReload;
        if (troopList != null) troopList.SelectionChanged += OnTroopSelected;

        // Populate category combo
        if (categoryCombo != null)
        {
            categoryCombo.ItemsSource = new[]
            {
                new { Value = UnitCategory.NotUsed, Name = "Not Used" },
                new { Value = UnitCategory.Local, Name = "Local (Player)" },
                new { Value = UnitCategory.Remote, Name = "Remote (Multiplayer)" },
                new { Value = UnitCategory.AiEnemy, Name = "AI Enemy" },
                new { Value = UnitCategory.AiFriendly, Name = "AI Friendly" },
                new { Value = UnitCategory.AiNeutral, Name = "AI Neutral" }
            };
            categoryCombo.DisplayMemberBinding = new Avalonia.Data.Binding("Name");
            categoryCombo.SelectedValueBinding = new Avalonia.Data.Binding("Value");
        }

        // Populate allegiance combo
        if (allegianceCombo != null)
        {
            allegianceCombo.ItemsSource = new[]
            {
                new { Value = UnitAllegiance.Ally, Name = "Ally" },
                new { Value = UnitAllegiance.Enemy, Name = "Enemy" },
                new { Value = UnitAllegiance.EnemyOfEveryone, Name = "Enemy of Everyone" }
            };
            allegianceCombo.DisplayMemberBinding = new Avalonia.Data.Binding("Name");
            allegianceCombo.SelectedValueBinding = new Avalonia.Data.Binding("Value");
        }
    }

    /// <summary>
    /// Loads a mission file.
    /// </summary>
    public void LoadFile(string path)
    {
        _filePath = path;

        try
        {
            _mission = MissionFile.Load(path);

            // Update file name display
            var fileNameText = this.FindControl<TextBlock>("FileNameText");
            if (fileNameText != null)
                fileNameText.Text = Path.GetFileName(path);

            // Populate troop list
            var troopList = this.FindControl<ListBox>("TroopList");
            if (troopList != null)
                troopList.ItemsSource = _mission.Troops;

            // Load hex view
            LoadHexView(path);

            // Select first troop if available
            if (_mission.Troops.Count > 0 && troopList != null)
                troopList.SelectedIndex = 0;
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error loading mission: {ex.Message}");
            var fileNameText = this.FindControl<TextBlock>("FileNameText");
            if (fileNameText != null)
                fileNameText.Text = $"Error: {ex.Message}";
        }
    }

    private void LoadHexView(string path)
    {
        try
        {
            var hexDump = MissionFile.GetHexDump(path);
            var hexView = this.FindControl<TextBox>("HexView");
            if (hexView != null)
                hexView.Text = hexDump;
        }
        catch (Exception ex)
        {
            var hexView = this.FindControl<TextBox>("HexView");
            if (hexView != null)
                hexView.Text = $"Error loading hex view: {ex.Message}";
        }
    }

    private void OnTroopSelected(object? sender, SelectionChangedEventArgs e)
    {
        var troopList = sender as ListBox;
        _selectedTroop = troopList?.SelectedItem as TroopBlock;

        if (_selectedTroop == null) return;

        // Update basic info
        UpdateControl<TextBox>("InternalNameText", tb => tb.Text = _selectedTroop.InternalName);
        UpdateControl<TextBlock>("UniqueIdText", tb => tb.Text = $"0x{_selectedTroop.UniqueId:X2}");
        UpdateControl<CheckBox>("IsHeroCheck", cb => cb.IsChecked = _selectedTroop.IsHero);
        UpdateControl<CheckBox>("IsEnabledCheck", cb => cb.IsChecked = _selectedTroop.IsEnabled);

        // Category and allegiance combos
        UpdateControl<ComboBox>("CategoryCombo", cb =>
        {
            for (int i = 0; i < cb.Items.Count; i++)
            {
                dynamic item = cb.Items[i]!;
                if ((UnitCategory)item.Value == _selectedTroop.Category)
                {
                    cb.SelectedIndex = i;
                    break;
                }
            }
        });

        UpdateControl<ComboBox>("AllegianceCombo", cb =>
        {
            for (int i = 0; i < cb.Items.Count; i++)
            {
                dynamic item = cb.Items[i]!;
                if ((UnitAllegiance)item.Value == _selectedTroop.Allegiance)
                {
                    cb.SelectedIndex = i;
                    break;
                }
            }
        });

        // Position
        UpdateControl<NumericUpDown>("PositionX", nud => nud.Value = (decimal)_selectedTroop.PositionX);
        UpdateControl<NumericUpDown>("PositionY", nud => nud.Value = (decimal)_selectedTroop.PositionY);
        UpdateControl<NumericUpDown>("Facing", nud => nud.Value = _selectedTroop.Facing);

        // HP overrides
        UpdateControl<NumericUpDown>("LeaderHP", nud => nud.Value = (decimal)_selectedTroop.LeaderHP);
        UpdateControl<NumericUpDown>("UnitHP", nud => nud.Value = (decimal)_selectedTroop.UnitHP);

        // Leader data
        UpdateControl<NumericUpDown>("LeaderAnimId", nud => nud.Value = _selectedTroop.Leader.AnimationId);
        UpdateControl<NumericUpDown>("LeaderModelId", nud => nud.Value = _selectedTroop.Leader.ModelId);
        UpdateControl<NumericUpDown>("LeaderLevel", nud => nud.Value = _selectedTroop.Leader.Level);

        // Leader skills
        UpdateLeaderSkills();

        // Unit data
        UpdateControl<NumericUpDown>("UnitAnimId", nud => nud.Value = _selectedTroop.TroopData.AnimationId);
        UpdateControl<NumericUpDown>("UnitModelId", nud => nud.Value = _selectedTroop.TroopData.ModelId);
        UpdateControl<NumericUpDown>("FormationId", nud => nud.Value = _selectedTroop.TroopData.FormationId);
        UpdateControl<NumericUpDown>("UnitX", nud => nud.Value = _selectedTroop.TroopData.UnitX);
        UpdateControl<NumericUpDown>("UnitY", nud => nud.Value = _selectedTroop.TroopData.UnitY);
        UpdateControl<TextBlock>("TotalUnits", tb => tb.Text = _selectedTroop.TroopData.TotalUnits.ToString());

        // Skill points
        UpdateControl<NumericUpDown>("SkillPoints", nud => nud.Value = (decimal)_selectedTroop.SkillPoints);

        // Flags
        UpdateControl<NumericUpDown>("FlagBearerId", nud => nud.Value = _selectedTroop.FlagBearerModel);
        UpdateControl<NumericUpDown>("FlagModelId", nud => nud.Value = _selectedTroop.FlagModel);
    }

    private void UpdateLeaderSkills()
    {
        if (_selectedTroop == null) return;

        var skills = new ObservableCollection<SkillSlotViewModel>();
        for (int i = 0; i < _selectedTroop.Leader.Skills.Length; i++)
        {
            var skill = _selectedTroop.Leader.Skills[i];
            skills.Add(new SkillSlotViewModel
            {
                Slot = i + 1,
                SkillId = skill.SkillId,
                SkillName = SkillIds.GetName(skill.SkillId),
                SkillLevel = skill.SkillLevel
            });
        }

        var grid = this.FindControl<DataGrid>("LeaderSkillsGrid");
        if (grid != null)
            grid.ItemsSource = skills;
    }

    private void UpdateControl<T>(string name, Action<T> action) where T : Control
    {
        var control = this.FindControl<T>(name);
        if (control != null)
            action(control);
    }

    private void OnSave(object? sender, RoutedEventArgs e)
    {
        if (_mission == null || string.IsNullOrEmpty(_filePath))
        {
            Console.WriteLine("No mission loaded");
            return;
        }

        try
        {
            // Save current troop properties before saving
            SaveCurrentTroopProperties();

            MissionFile.Save(_mission);
            Console.WriteLine("Mission saved successfully");

            // Reload hex view to show changes
            LoadHexView(_filePath);
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error saving: {ex.Message}");
        }
    }

    private void SaveCurrentTroopProperties()
    {
        if (_selectedTroop == null) return;

        // Read values from UI controls back to the selected troop
        _selectedTroop.InternalName = this.FindControl<TextBox>("InternalNameText")?.Text ?? _selectedTroop.InternalName;
        _selectedTroop.IsHero = this.FindControl<CheckBox>("IsHeroCheck")?.IsChecked ?? _selectedTroop.IsHero;
        _selectedTroop.IsEnabled = this.FindControl<CheckBox>("IsEnabledCheck")?.IsChecked ?? _selectedTroop.IsEnabled;

        // Category
        var categoryCombo = this.FindControl<ComboBox>("CategoryCombo");
        if (categoryCombo?.SelectedItem != null)
        {
            dynamic item = categoryCombo.SelectedItem;
            _selectedTroop.Category = (UnitCategory)item.Value;
        }

        // Allegiance
        var allegianceCombo = this.FindControl<ComboBox>("AllegianceCombo");
        if (allegianceCombo?.SelectedItem != null)
        {
            dynamic item = allegianceCombo.SelectedItem;
            _selectedTroop.Allegiance = (UnitAllegiance)item.Value;
        }

        // Position and HP
        _selectedTroop.PositionX = (float)(this.FindControl<NumericUpDown>("PositionX")?.Value ?? (decimal)_selectedTroop.PositionX);
        _selectedTroop.PositionY = (float)(this.FindControl<NumericUpDown>("PositionY")?.Value ?? (decimal)_selectedTroop.PositionY);
        _selectedTroop.Facing = (byte)(this.FindControl<NumericUpDown>("Facing")?.Value ?? _selectedTroop.Facing);
        _selectedTroop.LeaderHP = (float)(this.FindControl<NumericUpDown>("LeaderHP")?.Value ?? (decimal)_selectedTroop.LeaderHP);
        _selectedTroop.UnitHP = (float)(this.FindControl<NumericUpDown>("UnitHP")?.Value ?? (decimal)_selectedTroop.UnitHP);
        _selectedTroop.SkillPoints = (float)(this.FindControl<NumericUpDown>("SkillPoints")?.Value ?? (decimal)_selectedTroop.SkillPoints);
    }

    private void OnReload(object? sender, RoutedEventArgs e)
    {
        if (!string.IsNullOrEmpty(_filePath))
            LoadFile(_filePath);
    }
}

/// <summary>
/// View model for skill slots in the data grid.
/// </summary>
public class SkillSlotViewModel
{
    public int Slot { get; set; }
    public byte SkillId { get; set; }
    public string SkillName { get; set; } = string.Empty;
    public byte SkillLevel { get; set; }
}
