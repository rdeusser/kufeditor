# Patch imgui-node-editor for compatibility with latest Dear ImGui (docking branch).
# ImRect::Floor() was removed; replace with equivalent ImFloor() calls on Min/Max.

file(READ imgui_node_editor.cpp SRC)

# Replace all occurrences of expr.Floor() where expr is a member access ending in
# m_Bounds, m_GroupBounds, m_NodeRect, newBounds, etc.
# The pattern "identifier.Floor();" becomes "identifier.Min = ImFloor(identifier.Min); identifier.Max = ImFloor(identifier.Max);"
string(REGEX REPLACE
    "([a-zA-Z_>]+->m_Bounds)\\.Floor\\(\\)"
    "\\1.Min = ImFloor(\\1.Min); \\1.Max = ImFloor(\\1.Max)"
    SRC "${SRC}")
string(REGEX REPLACE
    "([a-zA-Z_>]+->m_GroupBounds)\\.Floor\\(\\)"
    "\\1.Min = ImFloor(\\1.Min); \\1.Max = ImFloor(\\1.Max)"
    SRC "${SRC}")
string(REGEX REPLACE
    "(m_NodeRect)\\.Floor\\(\\)"
    "\\1.Min = ImFloor(\\1.Min); \\1.Max = ImFloor(\\1.Max)"
    SRC "${SRC}")
string(REGEX REPLACE
    "(m_CurrentPin->m_Bounds)\\.Floor\\(\\)"
    "\\1.Min = ImFloor(\\1.Min); \\1.Max = ImFloor(\\1.Max)"
    SRC "${SRC}")
string(REGEX REPLACE
    "(m_GroupBounds)\\.Floor\\(\\)"
    "\\1.Min = ImFloor(\\1.Min); \\1.Max = ImFloor(\\1.Max)"
    SRC "${SRC}")
string(REGEX REPLACE
    "(newBounds)\\.Floor\\(\\)"
    "\\1.Min = ImFloor(\\1.Min); \\1.Max = ImFloor(\\1.Max)"
    SRC "${SRC}")

file(WRITE imgui_node_editor.cpp "${SRC}")
