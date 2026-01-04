#include "core/window.h"

int main() {
    kuf::Window window("KUF Editor", 1280, 720);

    while (!window.shouldClose()) {
        window.pollEvents();
        window.swapBuffers();
    }

    return 0;
}
