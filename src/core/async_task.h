#pragma once

#include <atomic>
#include <functional>
#include <mutex>
#include <string>
#include <thread>

namespace kuf {

enum class AsyncTaskState { Idle, Running, Completed, Failed };

class AsyncTask {
public:
    using TaskFn = std::function<bool(AsyncTask& self)>;

    ~AsyncTask() { reset(); }

    void start(TaskFn fn) {
        reset();
        state_.store(AsyncTaskState::Running);
        progress_.store(0.0f);
        {
            std::lock_guard lock(mutex_);
            status_ = "";
            error_ = "";
        }
        thread_ = std::thread([this, fn = std::move(fn)]() {
            try {
                bool ok = fn(*this);
                state_.store(ok ? AsyncTaskState::Completed : AsyncTaskState::Failed);
            } catch (const std::exception& e) {
                setError(e.what());
                state_.store(AsyncTaskState::Failed);
            } catch (...) {
                setError("Unknown error");
                state_.store(AsyncTaskState::Failed);
            }
        });
    }

    void setProgress(float value, const std::string& statusText) {
        progress_.store(value);
        std::lock_guard lock(mutex_);
        status_ = statusText;
    }

    void setError(const std::string& msg) {
        std::lock_guard lock(mutex_);
        error_ = msg;
    }

    AsyncTaskState state() const { return state_.load(); }
    float progress() const { return progress_.load(); }

    std::string status() const {
        std::lock_guard lock(mutex_);
        return status_;
    }

    std::string error() const {
        std::lock_guard lock(mutex_);
        return error_;
    }

    void reset() {
        if (thread_.joinable()) {
            thread_.join();
        }
        state_.store(AsyncTaskState::Idle);
        progress_.store(0.0f);
        std::lock_guard lock(mutex_);
        status_.clear();
        error_.clear();
    }

private:
    std::atomic<AsyncTaskState> state_{AsyncTaskState::Idle};
    std::atomic<float> progress_{0.0f};
    mutable std::mutex mutex_;
    std::string status_;
    std::string error_;
    std::thread thread_;
};

} // namespace kuf
