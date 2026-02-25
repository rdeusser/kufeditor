#include "ui/views/home_view.h"

#include <imgui.h>

namespace kuf {

HomeView::HomeView() : View("Home") {}

void HomeView::drawContent() {
	ImGui::Indent(8.0f);
	ImGui::Spacing();
	ImGui::TextWrapped("Welcome to KUF Editor. Use File > Open (Ctrl+O) to "
			   "open game files.");
	ImGui::Spacing();
	ImGui::TextDisabled("Configure game paths in Edit > Settings > Games.");
	ImGui::Unindent(8.0f);
}

} // namespace kuf
