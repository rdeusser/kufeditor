namespace KUFEditor.Core.UndoRedo;

/// <summary>
/// Manages undo and redo operations with configurable history depth.
/// </summary>
public class UndoRedoManager
{
    private readonly Stack<IUndoableCommand> _undoStack;
    private readonly Stack<IUndoableCommand> _redoStack;
    private readonly int _maxHistory;

    /// <summary>
    /// Raised when the undo/redo state changes.
    /// </summary>
    public event EventHandler? StateChanged;

    /// <summary>
    /// Initializes a new instance with a maximum history size.
    /// </summary>
    public UndoRedoManager(int maxHistory = 100)
    {
        _maxHistory = maxHistory;
        _undoStack = new Stack<IUndoableCommand>();
        _redoStack = new Stack<IUndoableCommand>();
    }

    /// <summary>
    /// Gets whether there are commands that can be undone.
    /// </summary>
    public bool CanUndo => _undoStack.Count > 0;

    /// <summary>
    /// Gets whether there are commands that can be redone.
    /// </summary>
    public bool CanRedo => _redoStack.Count > 0;

    /// <summary>
    /// Gets the description of the next undo command.
    /// </summary>
    public string? UndoDescription => _undoStack.Count > 0 ? _undoStack.Peek().Description : null;

    /// <summary>
    /// Gets the description of the next redo command.
    /// </summary>
    public string? RedoDescription => _redoStack.Count > 0 ? _redoStack.Peek().Description : null;

    /// <summary>
    /// Gets the number of commands in the undo stack.
    /// </summary>
    public int UndoCount => _undoStack.Count;

    /// <summary>
    /// Gets the number of commands in the redo stack.
    /// </summary>
    public int RedoCount => _redoStack.Count;

    /// <summary>
    /// Executes a command and adds it to the undo stack.
    /// </summary>
    public void Execute(IUndoableCommand command)
    {
        command.Execute();
        _undoStack.Push(command);
        _redoStack.Clear();

        TrimHistory();
        OnStateChanged();
    }

    /// <summary>
    /// Undoes the last command.
    /// </summary>
    public bool Undo()
    {
        if (!CanUndo)
            return false;

        var command = _undoStack.Pop();
        command.Undo();
        _redoStack.Push(command);

        OnStateChanged();
        return true;
    }

    /// <summary>
    /// Redoes the last undone command.
    /// </summary>
    public bool Redo()
    {
        if (!CanRedo)
            return false;

        var command = _redoStack.Pop();
        command.Execute();
        _undoStack.Push(command);

        OnStateChanged();
        return true;
    }

    /// <summary>
    /// Clears all undo/redo history.
    /// </summary>
    public void Clear()
    {
        _undoStack.Clear();
        _redoStack.Clear();
        OnStateChanged();
    }

    private void TrimHistory()
    {
        while (_undoStack.Count > _maxHistory)
        {
            var items = _undoStack.ToArray();
            _undoStack.Clear();
            for (int i = 0; i < items.Length - 1; i++)
            {
                _undoStack.Push(items[items.Length - 1 - i]);
            }
        }
    }

    private void OnStateChanged()
    {
        StateChanged?.Invoke(this, EventArgs.Empty);
    }
}
