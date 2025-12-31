namespace KUFEditor.Core.UndoRedo;

/// <summary>
/// Represents an undoable/redoable command.
/// </summary>
public interface IUndoableCommand
{
    /// <summary>
    /// Gets the description of this command for display in UI.
    /// </summary>
    string Description { get; }

    /// <summary>
    /// Executes the command.
    /// </summary>
    void Execute();

    /// <summary>
    /// Undoes the command.
    /// </summary>
    void Undo();
}
