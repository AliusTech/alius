/**
 * @file alius.h
 * @brief Alius Embedded SDK - C API
 *
 * This header provides a C-compatible interface for integrating Alius
 * into embedded systems (ESP32 with FreeRTOS, STM32 with RTOS).
 *
 * @version 0.1.0
 */

#ifndef ALIUS_H
#define ALIUS_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

/**
 * @brief Error codes for Alius operations
 */
typedef enum {
    ALIUS_SUCCESS = 0,            ///< Operation succeeded
    ALIUS_INVALID_ARGUMENT = 1,   ///< Invalid argument provided
    ALIUS_RUNTIME_ERROR = 2,      ///< Runtime error occurred
    ALIUS_NETWORK_ERROR = 3,      ///< Network error occurred
    ALIUS_MODEL_ERROR = 4,        ///< Model-related error
    ALIUS_CONFIG_ERROR = 6,       ///< Configuration error
    ALIUS_CANCELLED = 7,          ///< Operation was cancelled
    ALIUS_UNKNOWN = -1,           ///< Unknown error
} alius_error_code_t;

/**
 * @brief Error structure for returning error information
 */
typedef struct {
    alius_error_code_t code;       ///< Error code
    char* message;                 ///< Error message (allocated, must be freed)
} alius_error_t;

/**
 * @brief Stream callback function type
 *
 * Called for each chunk of the streaming response from the LLM.
 *
 * @param delta Null-terminated string containing the response chunk
 * @param user_data User-provided data pointer
 */
typedef void (*alius_stream_callback_t)(const char* delta, void* user_data);

/**
 * @brief Error callback function type
 *
 * Called when an error occurs during chat operations.
 *
 * @param code Error code
 * @param message Null-terminated error message
 * @param user_data User-provided data pointer
 */
typedef void (*alius_error_callback_t)(int code, const char* message, void* user_data);

/**
 * @brief Initialize the global Alius runtime
 *
 * Must be called before any other Alius functions. Can be called multiple times
 * (subsequent calls will no-op if already initialized).
 *
 * @return 0 on success, negative value on failure
 *
 * Example:
 * @code
 * if (alius_init() != 0) {
 *     printf("Failed to initialize Alius\\n");
 *     return -1;
 * }
 * @endcode
 */
int alius_init(void);

/**
 * @brief Cleanup the global Alius runtime
 *
 * Frees all resources allocated by the runtime. After calling this,
 * you must call alius_init() again before using any other functions.
 *
 * Example:
 * @code
 * alius_cleanup();
 * @endcode
 */
void alius_cleanup(void);

/**
 * @brief Configure the LLM model settings
 *
 * Sets the provider, model, and optional API key for LLM calls.
 *
 * @param provider Provider name (e.g., "anthropic", "openai")
 * @param model Model name (e.g., "claude-haiku-4-20250218", "gpt-4o-mini")
 * @param api_key API key (NULL if already configured)
 * @return alius_error_t with code=ALIUS_SUCCESS on success
 *
 * Example:
 * @code
 * alius_error_t err = alius_config_set_model(
 *     "anthropic",
 *     "claude-haiku-4-20250218",
 *     "your-api-key-here"
 * );
 * if (err.code != ALIUS_SUCCESS) {
 *     printf("Config error: %s\\n", err.message);
 *     alius_error_free(&err);
 * }
 * @endcode
 */
alius_error_t alius_config_set_model(
    const char* provider,
    const char* model,
    const char* api_key
);

/**
 * @brief Start a chat session with streaming responses
 *
 * Sends a message to the LLM and receives the response in chunks via callbacks.
 *
 * @param message Null-terminated message to send
 * @param stream_callback Called for each response chunk
 * @param error_callback Called on errors
 * @param user_data User data passed to callbacks
 * @return 0 on success, negative value on failure
 *
 * Example:
 * @code
 * void on_delta(const char* delta, void* user_data) {
 *     printf("%s", delta);
 * }
 *
 * void on_error(int code, const char* message, void* user_data) {
 *     printf("Error %d: %s\\n", code, message);
 * }
 *
 * alius_chat(
 *     "Hello from ESP32!",
 *     on_delta,
 *     on_error,
 *     NULL
 * );
 * @endcode
 */
int alius_chat(
    const char* message,
    alius_stream_callback_t stream_callback,
    alius_error_callback_t error_callback,
    void* user_data
);

/**
 * @brief Cancel the active chat session
 *
 * Stops the current streaming response. This is asynchronous and
 * the stream callback may still be called once after this.
 *
 * Example:
 * @code
 * alius_chat_cancel();
 * @endcode
 */
void alius_chat_cancel(void);

/**
 * @brief Get the Alius SDK version string
 *
 * @return Null-terminated version string (must NOT be freed)
 */
const char* alius_version(void);

/**
 * @brief Check runtime health
 *
 * Verifies that the runtime is properly configured and can reach
 * the LLM API.
 *
 * @return alius_error_t with code=ALIUS_SUCCESS if healthy
 *
 * Example:
 * @code
 * alius_error_t health = alius_health_check();
 * if (health.code != ALIUS_SUCCESS) {
 *     printf("Health check failed: %s\\n", health.message);
 * }
 * alius_error_free(&health);
 * @endcode
 */
alius_error_t alius_health_check(void);

/**
 * @brief Free an error structure's message
 *
 * Must be called for any non-NULL error.message to prevent memory leaks.
 *
 * @param error Pointer to error structure to free
 *
 * Example:
 * @code
 * alius_error_t err = some_function();
 * if (err.message != NULL) {
 *     alius_error_free(&err);
 * }
 * @endcode
 */
void alius_error_free(alius_error_t* error);

/**
 * @brief Free a string allocated by Alius
 *
 * Must be called for any strings returned by Alius that are documented
 * as requiring freeing.
 *
 * @param s String to free (NULL is safe)
 *
 * Example:
 * @code
 * char* version = strdup(alius_version());
 * // ... use version ...
 * alius_string_free(version);
 * @endcode
 */
void alius_string_free(char* s);

#ifdef __cplusplus
}
#endif

#endif // ALIUS_H
