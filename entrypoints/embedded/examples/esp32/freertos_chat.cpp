/**
 * @file freertos_chat.cpp
 * @brief Alius Embedded SDK Example - ESP32 with FreeRTOS
 *
 * This example demonstrates how to use the Alius Embedded SDK on ESP32
 * with FreeRTOS for streaming chat responses.
 *
 * Hardware Requirements:
 * - ESP32-WROVER or ESP32-S3 (recommended for more RAM)
 * - WiFi connection
 *
 * Build Instructions:
 * @code
 * cd entrypoints/embedded
 * cargo build --release --features esp32 --target xtensa-esp32-espidf
 * cargo espflash flash --release --features esp32
 * cargo espflash monitor
 * @endcode
 */

#include "alius.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_wifi.h"
#include "esp_event.h"
#include "esp_log.h"
#include "nvs_flash.h"

static const char* TAG = "ALIUS_EXAMPLE";

// WiFi credentials - UPDATE THESE
#define WIFI_SSID "your-wifi-ssid"
#define WIFI_PASS "your-wifi-password"

// API key - UPDATE THIS
#define ALIUS_API_KEY "your-api-key-here"

/**
 * @brief Stream callback - called for each chunk of response
 */
void stream_callback(const char* delta, void* user_data) {
    ESP_LOGI(TAG, "Delta: %s", delta);
    // In a real application, you might:
    // - Send to a display
    // - Process the text further
    // - Store in a buffer
}

/**
 * @brief Error callback - called on errors
 */
void error_callback(int code, const char* message, void* user_data) {
    ESP_LOGE(TAG, "Error %d: %s", code, message);
}

/**
 * @brief WiFi event handler
 */
static void wifi_event_handler(void* arg, esp_event_base_t event_base,
                                int32_t event_id, void* event_data) {
    if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_START) {
        esp_wifi_connect();
    } else if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_DISCONNECTED) {
        esp_wifi_connect();
        ESP_LOGI(TAG, "Connect to the AP failed");
    } else if (event_base == IP_EVENT && event_id == IP_EVENT_STA_GOT_IP) {
        ip_event_got_ip_t* event = (ip_event_got_ip_t*) event_data;
        ESP_LOGI(TAG, "Got IP:" IPSTR, IP2STR(&event->ip_info.ip));
    }
}

/**
 * @brief Initialize WiFi
 */
void wifi_init(void) {
    ESP_LOGI(TAG, "Initializing WiFi...");

    // Initialize NVS
    esp_err_t ret = nvs_flash_init();
    if (ret == ESP_ERR_NVS_NO_FREE_PAGES || ret == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        ret = nvs_flash_init();
    }
    ESP_ERROR_CHECK(ret);

    // Initialize TCP/IP
    ESP_ERROR_CHECK(esp_netif_init());
    ESP_ERROR_CHECK(esp_event_loop_create_default());
    esp_netif_create_default_wifi_sta();

    // Initialize WiFi
    wifi_init_config_t cfg = WIFI_INIT_CONFIG_DEFAULT();
    ESP_ERROR_CHECK(esp_wifi_init(&cfg));

    esp_event_handler_instance_t instance_any_id;
    esp_event_handler_instance_t instance_got_ip;
    ESP_ERROR_CHECK(esp_event_handler_instance_register(WIFI_EVENT,
                                                        ESP_EVENT_ANY_ID,
                                                        &wifi_event_handler,
                                                        NULL,
                                                        &instance_any_id));
    ESP_ERROR_CHECK(esp_event_handler_instance_register(IP_EVENT,
                                                        IP_EVENT_STA_GOT_IP,
                                                        &wifi_event_handler,
                                                        NULL,
                                                        &instance_got_ip));

    // Configure WiFi
    wifi_config_t wifi_config = {
        .sta = {
            .ssid = WIFI_SSID,
            .password = WIFI_PASS,
            .threshold.authmode = WIFI_AUTH_WPA2_PSK,
        },
    };
    ESP_ERROR_CHECK(esp_wifi_set_mode(WIFI_MODE_STA));
    ESP_ERROR_CHECK(esp_wifi_set_config(WIFI_IF_STA, &wifi_config));
    ESP_ERROR_CHECK(esp_wifi_start());

    ESP_LOGI(TAG, "WiFi initialization complete");
}

/**
 * @brief FreeRTOS task for chat operations
 */
void chat_task(void* pvParameters) {
    ESP_LOGI(TAG, "Starting chat task...");

    // Initialize Alius runtime
    if (alius_init() != 0) {
        ESP_LOGE(TAG, "Failed to initialize Alius");
        vTaskDelete(NULL);
        return;
    }
    ESP_LOGI(TAG, "Alius initialized");

    // Check health
    alius_error_t health = alius_health_check();
    if (health.code != ALIUS_SUCCESS) {
        ESP_LOGE(TAG, "Health check failed: %s", health.message ? health.message : "unknown");
        alius_error_free(&health);
        alius_cleanup();
        vTaskDelete(NULL);
        return;
    }
    ESP_LOGI(TAG, "Health check passed");
    alius_error_free(&health);

    // Configure LLM (use Haiku for ESP32 due to resource constraints)
    ESP_LOGI(TAG, "Configuring model...");
    alius_error_t config_err = alius_config_set_model(
        "anthropic",
        "claude-haiku-4-20250218",
        ALIUS_API_KEY
    );

    if (config_err.code != ALIUS_SUCCESS) {
        ESP_LOGE(TAG, "Config failed: %s", config_err.message ? config_err.message : "unknown");
        alius_error_free(&config_err);
        alius_cleanup();
        vTaskDelete(NULL);
        return;
    }
    ESP_LOGI(TAG, "Model configured");
    alius_error_free(&config_err);

    // Small delay to ensure WiFi is fully connected
    vTaskDelay(pdMS_TO_TICKS(2000));

    // Start chat with streaming
    ESP_LOGI(TAG, "Starting chat...");
    const char* message = "Hello from ESP32! Keep your response brief.";

    int chat_result = alius_chat(
        message,
        stream_callback,
        error_callback,
        NULL
    );

    if (chat_result != 0) {
        ESP_LOGE(TAG, "Chat failed with code: %d", chat_result);
    } else {
        ESP_LOGI(TAG, "Chat initiated successfully");
    }

    // Wait for response (streaming happens in background)
    vTaskDelay(pdMS_TO_TICKS(15000));

    // Cleanup
    ESP_LOGI(TAG, "Cleaning up...");
    alius_cleanup();

    vTaskDelete(NULL);
}

/**
 * @brief Application entry point
 */
extern "C" void app_main() {
    ESP_LOGI(TAG, "Alius Embedded SDK Example");
    ESP_LOGI(TAG, "Chip: %s, %d CPU cores, %s MHz",
             CONFIG_IDF_TARGET,
             CONFIG_ESP32_CPU_CORES_NUM,
             CONFIG_ESP32_XTAL_FREQ / 1000000);

    // Check version
    ESP_LOGI(TAG, "Alius SDK Version: %s", alius_version());

    // Initialize WiFi
    wifi_init();

    // Wait for WiFi connection
    vTaskDelay(pdMS_TO_TICKS(5000));

    // Create chat task
    xTaskCreate(
        chat_task,
        "chat_task",
        8192,           // Stack size
        NULL,           // Parameters
        5,              // Priority
        NULL            // Handle
    );
}
