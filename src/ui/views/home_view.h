#pragma once

#include "ui/views/view.h"

namespace kuf {

class HomeView : public View {
      public:
	HomeView();

	void drawContent() override;
};

} // namespace kuf
