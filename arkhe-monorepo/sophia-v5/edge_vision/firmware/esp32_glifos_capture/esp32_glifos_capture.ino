// SOPHIA V5.0 — Edge Vision Firmware (ESP32-CAM, OV2640)
// Captura glifos fenícios/grego e envia para o agente local via MQTT/UDP
// (pub/sub) ou serial. Sem envio para nuvem: destino final é o homelab.
//
// Pinout padrão AI-Thinker ESP32-CAM (OV2640).
#define PWDN_GPIO_NUM  32
#define RESET_GPIO_NUM -1
#define XCLK_GPIO_NUM  0
#define SIOD_GPIO_NUM  26
#define SIOC_GPIO_NUM  27
#define Y9_GPIO_NUM    35
#define Y8_GPIO_NUM    34
#define Y7_GPIO_NUM    39
#define Y6_GPIO_NUM    36
#define Y5_GPIO_NUM    21
#define Y4_GPIO_NUM    19
#define Y3_GPIO_NUM    18
#define Y2_GPIO_NUM     5
#define VSYNC_GPIO_NUM 25
#define HREF_GPIO_NUM  23
#define PCLK_GPIO_NUM  22

#include "esp_camera.h"
#include "soc/soc.h"
#include "soc/rtc_cntl_reg.h"
#include <WiFi.h>
#include <PubSubClient.h>
#include <ArduinoJson.h>

// --- Configuração local (homelab) ---
const char* WIFI_SSID = "sophia-lab";
const char* WIFI_PASS = "changeme-local";
const char* MQTT_HOST = "192.168.1.10";
const int   MQTT_PORT = 1883;
const char* MQTT_TOPIC = "sophia/edge/detections";

WiFiClient wifi_client;
PubSubClient mqtt(wifi_client);

void setup_camera() {
  camera_config_t config;
  config.ledc_channel = LEDC_CHANNEL_0;
  config.ledc_timer = LEDC_TIMER_0;
  config.pin_d0 = Y2_GPIO_NUM;
  config.pin_d1 = Y3_GPIO_NUM;
  config.pin_d2 = Y4_GPIO_NUM;
  config.pin_d3 = Y5_GPIO_NUM;
  config.pin_d4 = Y6_GPIO_NUM;
  config.pin_d5 = Y7_GPIO_NUM;
  config.pin_d6 = Y8_GPIO_NUM;
  config.pin_d7 = Y9_GPIO_NUM;
  config.pin_xclk = XCLK_GPIO_NUM;
  config.pin_pclk = PCLK_GPIO_NUM;
  config.pin_vsync = VSYNC_GPIO_NUM;
  config.pin_href = HREF_GPIO_NUM;
  config.pin_sscb_sda = SIOD_GPIO_NUM;
  config.pin_sscb_scl = SIOC_GPIO_NUM;
  config.pin_pwdn = PWDN_GPIO_NUM;
  config.pin_reset = RESET_GPIO_NUM;
  config.xclk_freq_hz = 20000000;
  config.pixel_format = PIXFORMAT_GRAYSCALE;      // TinyML prefer greyscale
  config.frame_size = FRAMESIZE_96x96;            // entrada de modelo leve
  config.jpeg_quality = 12;
  config.fb_count = 1;

  esp_err_t err = esp_camera_init(&config);
  if (err != ESP_OK) {
    Serial.printf("[ESP32] Falha na camera: 0x%x. Restart em 3s.\n", err);
    delay(3000);
    ESP.restart();
  }
  Serial.println("[ESP32] Camera OV2640 inicializada.");
}

void setup_wifi_and_mqtt() {
  WRITE_PERI_REG(RTC_CNTL_BROWN_OUT_REG, 0); // desativa brownout
  WiFi.mode(WIFI_STA);
  WiFi.begin(WIFI_SSID, WIFI_PASS);
  int tries = 0;
  while (WiFi.status() != WL_CONNECTED && tries < 40) {
    delay(250);
    Serial.print(".");
  }
  Serial.printf("\n[ESP32] WiFi %s\n", WiFi.isConnected() ? "CONECTADO" : "FALHOU");
  mqtt.setServer(MQTT_HOST, MQTT_PORT);
}

void publish_frame() {
  camera_fb_t* fb = esp_camera_fb_get();
  if (!fb) {
    Serial.println("[ESP32] Falha na captura.");
    return;
  }

  // Scaffold: envia only manifest (largura/altura/ts) — em produção enviar
  // o frame ou embeddings extraídos por TinyML local.
  StaticJsonDocument<256> doc;
  JsonObject manifest = doc.createNestedObject("manifest");
  manifest["w"] = fb->width;
  manifest["h"] = fb->height;
  manifest["ts"] = millis();
  manifest["device"] = "esp32-cam-01";

  char buffer[256];
  size_t n = serializeJson(doc, buffer);
  bool sent = mqtt.connected() ? mqtt.publish(MQTT_TOPIC, buffer, n) : false;
  Serial.printf("[ESP32] frame %ix%i payload=%uB mqtt=%s\n",
                fb->width, fb->height, (unsigned)n, sent ? "OK" : "SKIP");

  // Alternativa local: despejar o frame em flash/serial para o Pi-sidecar.
  esp_camera_fb_return(fb);
}

void setup() {
  Serial.begin(115200);
  Serial.println("[ESP32] SOPHIA V5.0 Edge Vision boot.");

  setup_camera();
  setup_wifi_and_mqtt();

  if (!mqtt.connect("sophia-edge-01", "arkhe", "arkhe-local")) {
    Serial.println("[ESP32] MQTT falhou (mantem-se operando em modo local).");
  } else {
    Serial.println("[ESP32] MQTT conectado ao homelab.");
  }
}

void loop() {
  mqtt.loop();
  publish_frame();
  delay(1000); // 1 Hz — captura contínua de glifos
}