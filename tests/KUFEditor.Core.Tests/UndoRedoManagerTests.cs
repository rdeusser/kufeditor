using Xunit;
using KUFEditor.Core.UndoRedo;

namespace KUFEditor.Core.Tests;

public class UndoRedoManagerTests
{
    [Fact]
    public void CanUndo_InitiallyFalse()
    {
        var manager = new UndoRedoManager();

        Assert.False(manager.CanUndo);
    }

    [Fact]
    public void CanRedo_InitiallyFalse()
    {
        var manager = new UndoRedoManager();

        Assert.False(manager.CanRedo);
    }

    [Fact]
    public void Execute_AddsToUndoStack()
    {
        var manager = new UndoRedoManager();
        var command = new TestCommand("Test");

        manager.Execute(command);

        Assert.True(manager.CanUndo);
        Assert.Equal(1, manager.UndoCount);
    }

    [Fact]
    public void Execute_CallsCommandExecute()
    {
        var manager = new UndoRedoManager();
        var command = new TestCommand("Test");

        manager.Execute(command);

        Assert.True(command.WasExecuted);
    }

    [Fact]
    public void Execute_ClearsRedoStack()
    {
        var manager = new UndoRedoManager();
        var command1 = new TestCommand("First");
        var command2 = new TestCommand("Second");

        manager.Execute(command1);
        manager.Undo();
        Assert.True(manager.CanRedo);

        manager.Execute(command2);

        Assert.False(manager.CanRedo);
    }

    [Fact]
    public void Undo_MovesToRedoStack()
    {
        var manager = new UndoRedoManager();
        var command = new TestCommand("Test");
        manager.Execute(command);

        manager.Undo();

        Assert.False(manager.CanUndo);
        Assert.True(manager.CanRedo);
    }

    [Fact]
    public void Undo_CallsCommandUndo()
    {
        var manager = new UndoRedoManager();
        var command = new TestCommand("Test");
        manager.Execute(command);

        manager.Undo();

        Assert.True(command.WasUndone);
    }

    [Fact]
    public void Undo_ReturnsFalseWhenEmpty()
    {
        var manager = new UndoRedoManager();

        var result = manager.Undo();

        Assert.False(result);
    }

    [Fact]
    public void Redo_MovesToUndoStack()
    {
        var manager = new UndoRedoManager();
        var command = new TestCommand("Test");
        manager.Execute(command);
        manager.Undo();

        manager.Redo();

        Assert.True(manager.CanUndo);
        Assert.False(manager.CanRedo);
    }

    [Fact]
    public void Redo_CallsCommandExecute()
    {
        var manager = new UndoRedoManager();
        var command = new TestCommand("Test");
        manager.Execute(command);
        manager.Undo();
        command.Reset();

        manager.Redo();

        Assert.True(command.WasExecuted);
    }

    [Fact]
    public void Redo_ReturnsFalseWhenEmpty()
    {
        var manager = new UndoRedoManager();

        var result = manager.Redo();

        Assert.False(result);
    }

    [Fact]
    public void Clear_EmptiesBothStacks()
    {
        var manager = new UndoRedoManager();
        manager.Execute(new TestCommand("One"));
        manager.Execute(new TestCommand("Two"));
        manager.Undo();

        manager.Clear();

        Assert.False(manager.CanUndo);
        Assert.False(manager.CanRedo);
    }

    [Fact]
    public void UndoDescription_ReturnsLastCommandDescription()
    {
        var manager = new UndoRedoManager();
        manager.Execute(new TestCommand("First"));
        manager.Execute(new TestCommand("Second"));

        Assert.Equal("Second", manager.UndoDescription);
    }

    [Fact]
    public void RedoDescription_ReturnsLastUndoneDescription()
    {
        var manager = new UndoRedoManager();
        manager.Execute(new TestCommand("Test Command"));
        manager.Undo();

        Assert.Equal("Test Command", manager.RedoDescription);
    }

    [Fact]
    public void StateChanged_FiresOnExecute()
    {
        var manager = new UndoRedoManager();
        var eventFired = false;
        manager.StateChanged += (s, e) => eventFired = true;

        manager.Execute(new TestCommand("Test"));

        Assert.True(eventFired);
    }

    [Fact]
    public void StateChanged_FiresOnUndo()
    {
        var manager = new UndoRedoManager();
        manager.Execute(new TestCommand("Test"));
        var eventFired = false;
        manager.StateChanged += (s, e) => eventFired = true;

        manager.Undo();

        Assert.True(eventFired);
    }

    [Fact]
    public void StateChanged_FiresOnRedo()
    {
        var manager = new UndoRedoManager();
        manager.Execute(new TestCommand("Test"));
        manager.Undo();
        var eventFired = false;
        manager.StateChanged += (s, e) => eventFired = true;

        manager.Redo();

        Assert.True(eventFired);
    }

    [Fact]
    public void MaxHistory_TrimsOldCommands()
    {
        var manager = new UndoRedoManager(maxHistory: 3);

        manager.Execute(new TestCommand("1"));
        manager.Execute(new TestCommand("2"));
        manager.Execute(new TestCommand("3"));
        manager.Execute(new TestCommand("4"));

        Assert.Equal(3, manager.UndoCount);
    }

    [Fact]
    public void MultipleUndoRedo_WorksCorrectly()
    {
        var manager = new UndoRedoManager();
        manager.Execute(new TestCommand("1"));
        manager.Execute(new TestCommand("2"));
        manager.Execute(new TestCommand("3"));

        manager.Undo();
        manager.Undo();

        Assert.Equal(1, manager.UndoCount);
        Assert.Equal(2, manager.RedoCount);
        Assert.Equal("1", manager.UndoDescription);

        manager.Redo();

        Assert.Equal(2, manager.UndoCount);
        Assert.Equal(1, manager.RedoCount);
        Assert.Equal("2", manager.UndoDescription);
    }

    private class TestCommand : IUndoableCommand
    {
        public string Description { get; }
        public bool WasExecuted { get; private set; }
        public bool WasUndone { get; private set; }

        public TestCommand(string description)
        {
            Description = description;
        }

        public void Execute() => WasExecuted = true;
        public void Undo() => WasUndone = true;
        public void Reset() { WasExecuted = false; WasUndone = false; }
    }
}
