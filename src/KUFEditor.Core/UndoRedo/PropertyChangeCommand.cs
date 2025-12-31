using System.Reflection;

namespace KUFEditor.Core.UndoRedo;

/// <summary>
/// Command that changes a property value on an object.
/// </summary>
public class PropertyChangeCommand<T> : IUndoableCommand
{
    private readonly object _target;
    private readonly PropertyInfo _property;
    private readonly T _oldValue;
    private readonly T _newValue;
    private readonly string _description;

    /// <summary>
    /// Creates a property change command.
    /// </summary>
    public PropertyChangeCommand(object target, string propertyName, T oldValue, T newValue, string? description = null)
    {
        _target = target ?? throw new ArgumentNullException(nameof(target));
        _property = target.GetType().GetProperty(propertyName)
            ?? throw new ArgumentException($"Property '{propertyName}' not found on {target.GetType().Name}");
        _oldValue = oldValue;
        _newValue = newValue;
        _description = description ?? $"Change {propertyName}";
    }

    public string Description => _description;

    public void Execute()
    {
        _property.SetValue(_target, _newValue);
    }

    public void Undo()
    {
        _property.SetValue(_target, _oldValue);
    }
}

/// <summary>
/// Command that changes a property using an action delegate.
/// </summary>
public class ActionCommand : IUndoableCommand
{
    private readonly Action _execute;
    private readonly Action _undo;
    private readonly string _description;

    /// <summary>
    /// Creates a command with execute and undo actions.
    /// </summary>
    public ActionCommand(Action execute, Action undo, string description)
    {
        _execute = execute ?? throw new ArgumentNullException(nameof(execute));
        _undo = undo ?? throw new ArgumentNullException(nameof(undo));
        _description = description ?? throw new ArgumentNullException(nameof(description));
    }

    public string Description => _description;

    public void Execute() => _execute();

    public void Undo() => _undo();
}

/// <summary>
/// Command that groups multiple commands together.
/// </summary>
public class CompositeCommand : IUndoableCommand
{
    private readonly List<IUndoableCommand> _commands;
    private readonly string _description;

    /// <summary>
    /// Creates a composite command.
    /// </summary>
    public CompositeCommand(string description, params IUndoableCommand[] commands)
    {
        _description = description ?? throw new ArgumentNullException(nameof(description));
        _commands = commands?.ToList() ?? new List<IUndoableCommand>();
    }

    public string Description => _description;

    /// <summary>
    /// Adds a command to the composite.
    /// </summary>
    public void Add(IUndoableCommand command)
    {
        _commands.Add(command);
    }

    public void Execute()
    {
        foreach (var cmd in _commands)
        {
            cmd.Execute();
        }
    }

    public void Undo()
    {
        // undo in reverse order
        for (int i = _commands.Count - 1; i >= 0; i--)
        {
            _commands[i].Undo();
        }
    }
}
