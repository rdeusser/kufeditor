using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Linq;
using Avalonia.Controls;
using Avalonia.Interactivity;
using KUFEditor.Assets.TroopInfo;

namespace KUFEditor.UI.Views;

public partial class TroopInfoEditor : UserControl
{
    private TroopInfoSox? _troopInfoSox;
    private TroopInfo? _selectedTroop;
    private int _selectedTroopIndex = -1;
    private string? _filePath;
    private bool _hasChanges = false;
    private ObservableCollection<TroopGridItem> _troopItems = new();

    public TroopInfoEditor()
    {
        InitializeComponent();
        SetupUI();
    }

    private void SetupUI()
    {
        var troopGrid = this.FindControl<DataGrid>("TroopGrid");
        if (troopGrid != null)
        {
            troopGrid.ItemsSource = _troopItems;
            troopGrid.SelectionChanged += OnTroopSelectionChanged;
        }

        var searchBox = this.FindControl<TextBox>("SearchBox");
        if (searchBox != null)
        {
            searchBox.TextChanged += OnSearchTextChanged;
        }

        SetupResistanceSliders();
        SetupNumericInputs();
    }

    private void SetupResistanceSliders()
    {
        var resistTypes = new[] { "Melee", "Ranged", "Frontal", "Explosion", "Fire", "Ice", "Lightning", "Holy", "Curse", "Poison" };

        foreach (var resistType in resistTypes)
        {
            var slider = this.FindControl<Slider>($"Resist{resistType}Slider");
            var valueText = this.FindControl<TextBlock>($"Resist{resistType}Value");

            if (slider != null && valueText != null)
            {
                slider.ValueChanged += (sender, e) =>
                {
                    valueText.Text = slider.Value.ToString("F0");
                    MarkAsModified();
                };
            }
        }
    }

    private void SetupNumericInputs()
    {
        var inputs = new[]
        {
            "JobInput", "TypeIDInput", "BaseWidthInput", "SightRangeInput", "DamageDistributionInput",
            "MoveSpeedInput", "RotateRateInput", "MoveAccelerationInput", "MoveDecelerationInput",
            "MaxUnitSpeedMultiplierInput", "DirectAttackInput", "IndirectAttackInput", "DefenseInput",
            "AttackRangeMaxInput", "AttackRangeMinInput", "AttackFrontRangeInput",
            "DefaultUnitHPInput", "UnitHPLevUpInput", "FormationRandomInput",
            "DefaultUnitNumXInput", "DefaultUnitNumYInput",
            "Skill1IDInput", "Skill1PerLevelInput", "Skill2IDInput", "Skill2PerLevelInput",
            "Skill3IDInput", "Skill3PerLevelInput"
        };

        foreach (var inputName in inputs)
        {
            var input = this.FindControl<NumericUpDown>(inputName);
            if (input != null)
            {
                input.ValueChanged += (sender, e) => MarkAsModified();
            }
        }
    }

    public void LoadFile(string path)
    {
        try
        {
            _filePath = path;
            var reader = new TroopInfoSoxFile();
            _troopInfoSox = reader.Read(path) as TroopInfoSox;

            if (_troopInfoSox != null)
            {
                PopulateTroopGrid();
                UpdateStatus("File loaded successfully");
                _hasChanges = false;
            }
        }
        catch (Exception ex)
        {
            UpdateStatus($"Error loading file: {ex.Message}");
        }
    }

    private void PopulateTroopGrid()
    {
        _troopItems.Clear();

        if (_troopInfoSox == null)
            return;

        for (int i = 0; i < TroopInfoSox.TROOP_COUNT; i++)
        {
            var troop = _troopInfoSox.TroopInfos[i];
            _troopItems.Add(new TroopGridItem
            {
                Index = i,
                Name = TroopNames.GetName(i),
                Job = troop.Job,
                TypeID = troop.TypeID,
                HP = troop.DefaultUnitHP,
                Attack = troop.DirectAttack,
                Defense = troop.Defense
            });
        }
    }

    private void OnTroopSelectionChanged(object? sender, SelectionChangedEventArgs e)
    {
        var grid = sender as DataGrid;
        var selected = grid?.SelectedItem as TroopGridItem;

        if (selected != null && _troopInfoSox != null)
        {
            _selectedTroopIndex = selected.Index;
            _selectedTroop = _troopInfoSox.TroopInfos[_selectedTroopIndex];
            LoadTroopProperties();

            var nameHeader = this.FindControl<TextBlock>("TroopNameHeader");
            if (nameHeader != null)
                nameHeader.Text = $"Editing: {selected.Name}";

            var tabs = this.FindControl<TabControl>("PropertyTabs");
            if (tabs != null)
                tabs.IsVisible = true;
        }
    }

    private void LoadTroopProperties()
    {
        if (_selectedTroop == null)
            return;

        // Basic
        SetNumericValue("JobInput", _selectedTroop.Job);
        SetNumericValue("TypeIDInput", _selectedTroop.TypeID);
        SetNumericValue("BaseWidthInput", (decimal)_selectedTroop.BaseWidth);
        SetNumericValue("SightRangeInput", (decimal)_selectedTroop.SightRange);
        SetNumericValue("DamageDistributionInput", (decimal)_selectedTroop.DamageDistribution);

        // Movement
        SetNumericValue("MoveSpeedInput", (decimal)_selectedTroop.MoveSpeed);
        SetNumericValue("RotateRateInput", (decimal)_selectedTroop.RotateRate);
        SetNumericValue("MoveAccelerationInput", (decimal)_selectedTroop.MoveAcceleration);
        SetNumericValue("MoveDecelerationInput", (decimal)_selectedTroop.MoveDeceleration);
        SetNumericValue("MaxUnitSpeedMultiplierInput", (decimal)_selectedTroop.MaxUnitSpeedMultiplier);

        // Combat
        SetNumericValue("DirectAttackInput", (decimal)_selectedTroop.DirectAttack);
        SetNumericValue("IndirectAttackInput", (decimal)_selectedTroop.IndirectAttack);
        SetNumericValue("DefenseInput", (decimal)_selectedTroop.Defense);
        SetNumericValue("AttackRangeMaxInput", (decimal)_selectedTroop.AttackRangeMax);
        SetNumericValue("AttackRangeMinInput", (decimal)_selectedTroop.AttackRangeMin);
        SetNumericValue("AttackFrontRangeInput", (decimal)_selectedTroop.AttackFrontRange);

        // Resistances
        SetSliderValue("ResistMeleeSlider", _selectedTroop.ResistMelee);
        SetSliderValue("ResistRangedSlider", _selectedTroop.ResistRanged);
        SetSliderValue("ResistFrontalSlider", _selectedTroop.ResistFrontal);
        SetSliderValue("ResistExplosionSlider", _selectedTroop.ResistExplosion);
        SetSliderValue("ResistFireSlider", _selectedTroop.ResistFire);
        SetSliderValue("ResistIceSlider", _selectedTroop.ResistIce);
        SetSliderValue("ResistLightningSlider", _selectedTroop.ResistLightning);
        SetSliderValue("ResistHolySlider", _selectedTroop.ResistHoly);
        SetSliderValue("ResistCurseSlider", _selectedTroop.ResistCurse);
        SetSliderValue("ResistPoisonSlider", _selectedTroop.ResistPoison);

        // Unit Config
        SetNumericValue("DefaultUnitHPInput", (decimal)_selectedTroop.DefaultUnitHP);
        SetNumericValue("UnitHPLevUpInput", (decimal)_selectedTroop.UnitHPLevUp);
        SetNumericValue("FormationRandomInput", _selectedTroop.FormationRandom);
        SetNumericValue("DefaultUnitNumXInput", _selectedTroop.DefaultUnitNumX);
        SetNumericValue("DefaultUnitNumYInput", _selectedTroop.DefaultUnitNumY);

        // Level Up Data
        SetNumericValue("Skill1IDInput", _selectedTroop.LevelUpData[0].SkillID);
        SetNumericValue("Skill1PerLevelInput", (decimal)_selectedTroop.LevelUpData[0].SkillPerLevel);
        SetNumericValue("Skill2IDInput", _selectedTroop.LevelUpData[1].SkillID);
        SetNumericValue("Skill2PerLevelInput", (decimal)_selectedTroop.LevelUpData[1].SkillPerLevel);
        SetNumericValue("Skill3IDInput", _selectedTroop.LevelUpData[2].SkillID);
        SetNumericValue("Skill3PerLevelInput", (decimal)_selectedTroop.LevelUpData[2].SkillPerLevel);
    }

    private void SaveTroopProperties()
    {
        if (_selectedTroop == null)
            return;

        // Basic
        _selectedTroop.Job = (int)GetNumericValue("JobInput");
        _selectedTroop.TypeID = (int)GetNumericValue("TypeIDInput");
        _selectedTroop.BaseWidth = (float)GetNumericValue("BaseWidthInput");
        _selectedTroop.SightRange = (float)GetNumericValue("SightRangeInput");
        _selectedTroop.DamageDistribution = (float)GetNumericValue("DamageDistributionInput");

        // Movement
        _selectedTroop.MoveSpeed = (float)GetNumericValue("MoveSpeedInput");
        _selectedTroop.RotateRate = (float)GetNumericValue("RotateRateInput");
        _selectedTroop.MoveAcceleration = (float)GetNumericValue("MoveAccelerationInput");
        _selectedTroop.MoveDeceleration = (float)GetNumericValue("MoveDecelerationInput");
        _selectedTroop.MaxUnitSpeedMultiplier = (float)GetNumericValue("MaxUnitSpeedMultiplierInput");

        // Combat
        _selectedTroop.DirectAttack = (float)GetNumericValue("DirectAttackInput");
        _selectedTroop.IndirectAttack = (float)GetNumericValue("IndirectAttackInput");
        _selectedTroop.Defense = (float)GetNumericValue("DefenseInput");
        _selectedTroop.AttackRangeMax = (float)GetNumericValue("AttackRangeMaxInput");
        _selectedTroop.AttackRangeMin = (float)GetNumericValue("AttackRangeMinInput");
        _selectedTroop.AttackFrontRange = (float)GetNumericValue("AttackFrontRangeInput");

        // Resistances
        _selectedTroop.ResistMelee = (float)GetSliderValue("ResistMeleeSlider");
        _selectedTroop.ResistRanged = (float)GetSliderValue("ResistRangedSlider");
        _selectedTroop.ResistFrontal = (float)GetSliderValue("ResistFrontalSlider");
        _selectedTroop.ResistExplosion = (float)GetSliderValue("ResistExplosionSlider");
        _selectedTroop.ResistFire = (float)GetSliderValue("ResistFireSlider");
        _selectedTroop.ResistIce = (float)GetSliderValue("ResistIceSlider");
        _selectedTroop.ResistLightning = (float)GetSliderValue("ResistLightningSlider");
        _selectedTroop.ResistHoly = (float)GetSliderValue("ResistHolySlider");
        _selectedTroop.ResistCurse = (float)GetSliderValue("ResistCurseSlider");
        _selectedTroop.ResistPoison = (float)GetSliderValue("ResistPoisonSlider");

        // Unit Config
        _selectedTroop.DefaultUnitHP = (float)GetNumericValue("DefaultUnitHPInput");
        _selectedTroop.UnitHPLevUp = (float)GetNumericValue("UnitHPLevUpInput");
        _selectedTroop.FormationRandom = (int)GetNumericValue("FormationRandomInput");
        _selectedTroop.DefaultUnitNumX = (int)GetNumericValue("DefaultUnitNumXInput");
        _selectedTroop.DefaultUnitNumY = (int)GetNumericValue("DefaultUnitNumYInput");

        // Level Up Data
        _selectedTroop.LevelUpData[0].SkillID = (int)GetNumericValue("Skill1IDInput");
        _selectedTroop.LevelUpData[0].SkillPerLevel = (float)GetNumericValue("Skill1PerLevelInput");
        _selectedTroop.LevelUpData[1].SkillID = (int)GetNumericValue("Skill2IDInput");
        _selectedTroop.LevelUpData[1].SkillPerLevel = (float)GetNumericValue("Skill2PerLevelInput");
        _selectedTroop.LevelUpData[2].SkillID = (int)GetNumericValue("Skill3IDInput");
        _selectedTroop.LevelUpData[2].SkillPerLevel = (float)GetNumericValue("Skill3PerLevelInput");

        // Update grid
        if (_selectedTroopIndex >= 0 && _selectedTroopIndex < _troopItems.Count)
        {
            var item = _troopItems[_selectedTroopIndex];
            item.Job = _selectedTroop.Job;
            item.TypeID = _selectedTroop.TypeID;
            item.HP = _selectedTroop.DefaultUnitHP;
            item.Attack = _selectedTroop.DirectAttack;
            item.Defense = _selectedTroop.Defense;
        }
    }

    private void OnSave(object? sender, RoutedEventArgs e)
    {
        if (_troopInfoSox == null || string.IsNullOrEmpty(_filePath))
            return;

        try
        {
            SaveTroopProperties();
            var writer = new TroopInfoSoxFile();
            writer.Write(_filePath, _troopInfoSox);
            _hasChanges = false;
            UpdateStatus("File saved successfully");
        }
        catch (Exception ex)
        {
            UpdateStatus($"Error saving file: {ex.Message}");
        }
    }

    private void OnBackup(object? sender, RoutedEventArgs e)
    {
        if (string.IsNullOrEmpty(_filePath))
            return;

        try
        {
            string backupPath = _filePath + ".bak";
            File.Copy(_filePath, backupPath, true);
            UpdateStatus($"Backup created: {Path.GetFileName(backupPath)}");
        }
        catch (Exception ex)
        {
            UpdateStatus($"Error creating backup: {ex.Message}");
        }
    }

    private void OnRestore(object? sender, RoutedEventArgs e)
    {
        if (string.IsNullOrEmpty(_filePath))
            return;

        try
        {
            TroopInfoSoxFile.RestoreFromBackup(_filePath);
            LoadFile(_filePath);
            UpdateStatus("Restored from backup successfully");
        }
        catch (Exception ex)
        {
            UpdateStatus($"Error restoring from backup: {ex.Message}");
        }
    }

    private void OnResetSelected(object? sender, RoutedEventArgs e)
    {
        if (_selectedTroop != null && !string.IsNullOrEmpty(_filePath))
        {
            try
            {
                var reader = new TroopInfoSoxFile();
                var originalSox = reader.Read(_filePath) as TroopInfoSox;
                if (originalSox != null && _selectedTroopIndex >= 0)
                {
                    _troopInfoSox!.TroopInfos[_selectedTroopIndex] = originalSox.TroopInfos[_selectedTroopIndex];
                    _selectedTroop = _troopInfoSox.TroopInfos[_selectedTroopIndex];
                    LoadTroopProperties();
                    PopulateTroopGrid();
                    UpdateStatus("Reset selected troop");
                }
            }
            catch (Exception ex)
            {
                UpdateStatus($"Error resetting: {ex.Message}");
            }
        }
    }

    private void OnResetAll(object? sender, RoutedEventArgs e)
    {
        if (!string.IsNullOrEmpty(_filePath))
        {
            LoadFile(_filePath);
            UpdateStatus("Reset all troops");
        }
    }

    private void OnSearchTextChanged(object? sender, TextChangedEventArgs e)
    {
        var searchBox = sender as TextBox;
        if (searchBox == null)
            return;

        var searchText = searchBox.Text?.ToLower() ?? "";

        var troopGrid = this.FindControl<DataGrid>("TroopGrid");
        if (troopGrid == null)
            return;

        // Simple filtering - in production would use CollectionView
        if (string.IsNullOrWhiteSpace(searchText))
        {
            troopGrid.ItemsSource = _troopItems;
        }
        else
        {
            var filtered = _troopItems.Where(t =>
                t.Name.ToLower().Contains(searchText) ||
                t.Job.ToString().Contains(searchText) ||
                t.TypeID.ToString().Contains(searchText)).ToList();
            troopGrid.ItemsSource = filtered;
        }
    }

    private void SetNumericValue(string controlName, decimal value)
    {
        var control = this.FindControl<NumericUpDown>(controlName);
        if (control != null)
            control.Value = value;
    }

    private decimal GetNumericValue(string controlName)
    {
        var control = this.FindControl<NumericUpDown>(controlName);
        return control?.Value ?? 0;
    }

    private void SetSliderValue(string controlName, double value)
    {
        var control = this.FindControl<Slider>(controlName);
        if (control != null)
            control.Value = value;
    }

    private double GetSliderValue(string controlName)
    {
        var control = this.FindControl<Slider>(controlName);
        return control?.Value ?? 0;
    }

    private void MarkAsModified()
    {
        if (!_hasChanges)
        {
            _hasChanges = true;
            UpdateStatus("Modified");
        }
    }

    private void UpdateStatus(string message)
    {
        var statusText = this.FindControl<TextBlock>("StatusText");
        if (statusText != null)
            statusText.Text = message;
    }

    private class TroopGridItem : INotifyPropertyChanged
    {
        private int _index;
        private string _name = "";
        private int _job;
        private int _typeID;
        private float _hp;
        private float _attack;
        private float _defense;

        public int Index
        {
            get => _index;
            set { _index = value; OnPropertyChanged(nameof(Index)); }
        }

        public string Name
        {
            get => _name;
            set { _name = value; OnPropertyChanged(nameof(Name)); }
        }

        public int Job
        {
            get => _job;
            set { _job = value; OnPropertyChanged(nameof(Job)); }
        }

        public int TypeID
        {
            get => _typeID;
            set { _typeID = value; OnPropertyChanged(nameof(TypeID)); }
        }

        public float HP
        {
            get => _hp;
            set { _hp = value; OnPropertyChanged(nameof(HP)); }
        }

        public float Attack
        {
            get => _attack;
            set { _attack = value; OnPropertyChanged(nameof(Attack)); }
        }

        public float Defense
        {
            get => _defense;
            set { _defense = value; OnPropertyChanged(nameof(Defense)); }
        }

        public event PropertyChangedEventHandler? PropertyChanged;

        protected virtual void OnPropertyChanged(string propertyName)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        }
    }
}