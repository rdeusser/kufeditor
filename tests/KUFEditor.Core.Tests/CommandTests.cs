using Xunit;
using KUFEditor.Core.UndoRedo;

namespace KUFEditor.Core.Tests;

public class PropertyChangeCommandTests
{
    [Fact]
    public void Execute_SetsNewValue()
    {
        var target = new TestObject { Value = "old" };
        var command = new PropertyChangeCommand<string>(target, "Value", "old", "new");

        command.Execute();

        Assert.Equal("new", target.Value);
    }

    [Fact]
    public void Undo_RestoresOldValue()
    {
        var target = new TestObject { Value = "old" };
        var command = new PropertyChangeCommand<string>(target, "Value", "old", "new");
        command.Execute();

        command.Undo();

        Assert.Equal("old", target.Value);
    }

    [Fact]
    public void Description_UsesPropertyName()
    {
        var target = new TestObject();
        var command = new PropertyChangeCommand<string>(target, "Value", "old", "new");

        Assert.Contains("Value", command.Description);
    }

    [Fact]
    public void Description_UsesCustomDescription()
    {
        var target = new TestObject();
        var command = new PropertyChangeCommand<string>(target, "Value", "old", "new", "Custom Desc");

        Assert.Equal("Custom Desc", command.Description);
    }

    [Fact]
    public void Constructor_ThrowsOnNullTarget()
    {
        Assert.Throws<ArgumentNullException>(() =>
            new PropertyChangeCommand<string>(null!, "Value", "old", "new"));
    }

    [Fact]
    public void Constructor_ThrowsOnInvalidProperty()
    {
        var target = new TestObject();

        Assert.Throws<ArgumentException>(() =>
            new PropertyChangeCommand<string>(target, "NonExistent", "old", "new"));
    }

    [Fact]
    public void WorksWithNumericTypes()
    {
        var target = new TestObject { Number = 10 };
        var command = new PropertyChangeCommand<int>(target, "Number", 10, 20);

        command.Execute();
        Assert.Equal(20, target.Number);

        command.Undo();
        Assert.Equal(10, target.Number);
    }

    private class TestObject
    {
        public string Value { get; set; } = string.Empty;
        public int Number { get; set; }
    }
}

public class ActionCommandTests
{
    [Fact]
    public void Execute_CallsExecuteAction()
    {
        var executed = false;
        var command = new ActionCommand(
            () => executed = true,
            () => { },
            "Test");

        command.Execute();

        Assert.True(executed);
    }

    [Fact]
    public void Undo_CallsUndoAction()
    {
        var undone = false;
        var command = new ActionCommand(
            () => { },
            () => undone = true,
            "Test");

        command.Undo();

        Assert.True(undone);
    }

    [Fact]
    public void Description_ReturnsProvidedDescription()
    {
        var command = new ActionCommand(
            () => { },
            () => { },
            "My Description");

        Assert.Equal("My Description", command.Description);
    }

    [Fact]
    public void Constructor_ThrowsOnNullExecute()
    {
        Assert.Throws<ArgumentNullException>(() =>
            new ActionCommand(null!, () => { }, "Test"));
    }

    [Fact]
    public void Constructor_ThrowsOnNullUndo()
    {
        Assert.Throws<ArgumentNullException>(() =>
            new ActionCommand(() => { }, null!, "Test"));
    }
}

public class CompositeCommandTests
{
    [Fact]
    public void Execute_ExecutesAllCommands()
    {
        var values = new List<int>();
        var command = new CompositeCommand("Test",
            new ActionCommand(() => values.Add(1), () => values.Remove(1), "Add 1"),
            new ActionCommand(() => values.Add(2), () => values.Remove(2), "Add 2"),
            new ActionCommand(() => values.Add(3), () => values.Remove(3), "Add 3"));

        command.Execute();

        Assert.Equal(new[] { 1, 2, 3 }, values);
    }

    [Fact]
    public void Undo_UndoesInReverseOrder()
    {
        var order = new List<string>();
        var command = new CompositeCommand("Test",
            new ActionCommand(() => { }, () => order.Add("A"), "A"),
            new ActionCommand(() => { }, () => order.Add("B"), "B"),
            new ActionCommand(() => { }, () => order.Add("C"), "C"));

        command.Undo();

        Assert.Equal(new[] { "C", "B", "A" }, order);
    }

    [Fact]
    public void Add_AddsCommandToComposite()
    {
        var values = new List<int>();
        var command = new CompositeCommand("Test");
        command.Add(new ActionCommand(() => values.Add(1), () => { }, "Add 1"));
        command.Add(new ActionCommand(() => values.Add(2), () => { }, "Add 2"));

        command.Execute();

        Assert.Equal(new[] { 1, 2 }, values);
    }

    [Fact]
    public void Description_ReturnsProvidedDescription()
    {
        var command = new CompositeCommand("Composite Operation");

        Assert.Equal("Composite Operation", command.Description);
    }
}
