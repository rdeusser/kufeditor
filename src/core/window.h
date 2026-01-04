#pragma once

#include <string>
#include <string_view>

struct GLFWwindow;

namespace kuf {

class Window {
public:
    Window(std::string_view title, int width, int height);
    ~Window();

    Window(const Window&) = delete;
    Window& operator=(const Window&) = delete;

    bool shouldClose() const;
    void pollEvents();
    void swapBuffers();

    GLFWwindow* handle() const { return window_; }
    int width() const { return width_; }
    int height() const { return height_; }

private:
    GLFWwindow* window_ = nullptr;
    int width_;
    int height_;
};

} // namespace kuf
