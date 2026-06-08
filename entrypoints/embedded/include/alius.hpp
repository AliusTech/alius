/**
 * @file alius.hpp
 * @brief Alius Embedded SDK - C++ API
 *
 * Modern C++ wrapper with RAII and type safety for the Alius Embedded SDK.
 * Designed for ESP32 with FreeRTOS and other embedded platforms.
 *
 * @version 0.1.0
 */

#ifndef ALIUS_HPP
#define ALIUS_HPP

#include "alius.h"
#include <string>
#include <functional>
#include <stdexcept>

namespace alius {

/**
 * @brief Exception class for Alius errors
 */
class Exception : public std::runtime_error {
public:
    Exception(alius_error_code_t code, const std::string& message)
        : std::runtime_error(message), code_(code) {}

    alius_error_code_t code() const noexcept { return code_; }

private:
    alius_error_code_t code_;
};

/**
 * @brief Error wrapper with automatic cleanup
 */
class Error {
public:
    Error() : error_{alius_error_code_t::ALIUS_SUCCESS, nullptr} {}
    Error(const alius_error_t& error) : error_(error) {}
    ~Error() { alius_error_free(&error_); }

    // Non-copyable
    Error(const Error&) = delete;
    Error& operator=(const Error&) = delete;

    // Movable
    Error(Error&& other) noexcept : error_(other.error_) {
        other.error_.message = nullptr;
    }
    Error& operator=(Error&& other) noexcept {
        if (this != &other) {
            alius_error_free(&error_);
            error_ = other.error_;
            other.error_.message = nullptr;
        }
        return *this;
    }

    alius_error_code_t code() const noexcept { return error_.code; }
    const char* message() const noexcept { return error_.message; }

    bool is_success() const noexcept { return error_.code == alius_error_code_t::ALIUS_SUCCESS; }

    void throw_if_failed() const {
        if (!is_success()) {
            throw Exception(error_.code, error_.message ? error_.message : "Unknown error");
        }
    }

private:
    alius_error_t error_;
};

/**
 * @brief Stream callbacks configuration
 */
struct StreamCallbacks {
    std::function<void(const std::string&)> on_delta;
    std::function<void(int code, const std::string&)> on_error;
};

/**
 * @brief Model configuration
 */
struct ModelConfig {
    std::string provider = "anthropic";
    std::string model = "claude-haiku-4-20250218";
    std::string api_key;
};

/**
 * @brief Main runtime class for Alius Embedded SDK
 *
 * Provides RAII-managed access to Alius chat functionality.
 * Uses singleton pattern for embedded systems (single global instance).
 */
class Runtime {
public:
    /**
     * @brief Initialize the global runtime
     * @throw Exception if initialization fails
     */
    static void init() {
        if (alius_init() != 0) {
            throw Exception(alius_error_code_t::ALIUS_RUNTIME_ERROR, "Failed to initialize Alius");
        }
    }

    /**
     * @brief Cleanup the global runtime
     */
    static void cleanup() {
        alius_cleanup();
    }

    /**
     * @brief Get the singleton instance
     * @throw Exception if runtime not initialized
     */
    static Runtime& instance() {
        static Runtime inst;
        return inst;
    }

    /**
     * @brief Configure the LLM model
     * @param config Model configuration
     * @throw Exception on failure
     */
    void configure(const ModelConfig& config) {
        Error err = alius_config_set_model(
            config.provider.c_str(),
            config.model.c_str(),
            config.api_key.empty() ? nullptr : config.api_key.c_str()
        );
        err.throw_if_failed();
    }

    /**
     * @brief Send a chat message with streaming response
     * @param message Message to send
     * @param callbacks Stream callbacks
     * @throw Exception on failure
     */
    void chat(const std::string& message, const StreamCallbacks& callbacks) {
        user_data_ = callbacks;

        int result = alius_chat(
            message.c_str(),
            &Runtime::static_stream_callback,
            &Runtime::static_error_callback,
            this
        );

        if (result != 0) {
            throw Exception(alius_error_code_t::ALIUS_RUNTIME_ERROR, "Chat failed");
        }
    }

    /**
     * @brief Cancel the active chat session
     */
    void cancel() {
        alius_chat_cancel();
    }

    /**
     * @brief Get version string
     */
    static std::string version() {
        return alius_version();
    }

    /**
     * @brief Check runtime health
     * @throw Exception if unhealthy
     */
    void health_check() {
        Error err = alius_health_check();
        err.throw_if_failed();
    }

private:
    Runtime() = default;
    ~Runtime() = default;

    // Non-copyable, non-movable for singleton
    Runtime(const Runtime&) = delete;
    Runtime& operator=(const Runtime&) = delete;
    Runtime(Runtime&&) = delete;
    Runtime& operator=(Runtime&&) = delete;

    // Static callback wrappers
    static void static_stream_callback(const char* delta, void* user_data) {
        auto* runtime = static_cast<Runtime*>(user_data);
        if (runtime && runtime->user_data_.on_delta) {
            runtime->user_data_.on_delta(delta);
        }
    }

    static void static_error_callback(int code, const char* message, void* user_data) {
        auto* runtime = static_cast<Runtime*>(user_data);
        if (runtime && runtime->user_data_.on_error) {
            runtime->user_data_.on_error(code, message);
        }
    }

    StreamCallbacks user_data_;
};

/**
 * @brief Convenience class for simplified single-session API
 *
 * Provides static methods for common operations without managing
 * a Runtime instance.
 */
class EmbeddedClient {
public:
    /**
     * @brief Initialize Alius (calls Runtime::init)
     */
    static void init() {
        Runtime::init();
    }

    /**
     * @brief Cleanup Alius (calls Runtime::cleanup)
     */
    static void cleanup() {
        Runtime::cleanup();
    }

    /**
     * @brief Configure model
     */
    static void configureModel(const ModelConfig& config) {
        Runtime::instance().configure(config);
    }

    /**
     * @brief Send chat message
     */
    static void chat(const std::string& message, const StreamCallbacks& callbacks) {
        Runtime::instance().chat(message, callbacks);
    }

    /**
     * @brief Cancel chat
     */
    static void cancel() {
        Runtime::instance().cancel();
    }

    /**
     * @brief Get version
     */
    static std::string version() {
        return Runtime::version();
    }
};

} // namespace alius

#endif // ALIUS_HPP
