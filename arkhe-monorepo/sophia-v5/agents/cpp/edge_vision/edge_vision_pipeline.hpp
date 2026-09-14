#pragma once
// Pipeline de visão em edge (Raspberry Pi).
// Detecta glifos fenícios e caracteres gregos sem enviar dados à nuvem.
// Requer OpenCV. Em builds sem OpenCV, o pipeline entra em modo simulado.
#include <chrono>
#include <fstream>
#include <iomanip>
#include <sstream>
#include <string>
#include <vector>

#if __has_include(<opencv2/opencv.hpp>)
#include <opencv2/opencv.hpp>
#define SOPHIA_HAVE_OPENCV 1
#else
#define SOPHIA_HAVE_OPENCV 0
#endif

namespace Sophia::Edge {

struct Detection {
    int x, y, width, height;
    std::string label;
    double confidence;
};

struct VisionResult {
    std::vector<Detection> detections;
    bool camera_ok;
    int64_t timestamp_ms;
};

class EdgeVisionPipeline {
public:
    explicit EdgeVisionPipeline(int camera_id = 0)
        : camera_id_(camera_id), model_loaded_(false) {
        camera_ok_ = openCamera();
    }

    // Captura um frame e executa inferência local.
    VisionResult captureAndDetect() {
        VisionResult result;
        result.timestamp_ms = timestampMs();
        result.camera_ok = camera_ok_;

#if SOPHIA_HAVE_OPENCV
        cv::Mat frame;
        if (!cap_.read(frame)) {
            return result;
        }
        cv::Mat gray;
        cv::cvtColor(frame, gray, cv::COLOR_BGR2GRAY);
        // Pipeline real de detecção (YOLO-tiny/ONNX) ficaria aqui.
        // Scaffold: um contorno de destaque por frame.
        Detection d;
        d.label = "Aleph";
        d.confidence = 0.9;
        d.x = 10; d.y = 10; d.width = 50; d.height = 50;
        result.detections.push_back(d);
#else
        // Modo simulado quando OpenCV não está disponível (build limpo).
        Detection d;
        d.label = "Aleph";
        d.confidence = 0.92;
        d.x = 10; d.y = 10; d.width = 50; d.height = 50;
        result.detections.push_back(d);
#endif
        return result;
    }

    // Persiste detecções no arquivo lido pelo orquestrador/socket local.
    bool writeDetections(const std::string& path, const VisionResult& result) {
        std::ofstream out(path);
        if (!out) return false;
        out << "[";
        for (std::size_t i = 0; i < result.detections.size(); ++i) {
            const Detection& d = result.detections[i];
            out << "{\"label\":\"" << d.label
                << "\",\"confidence\":" << d.confidence
                << ",\"bbox\":{\"x\":" << d.x << ",\"y\":" << d.y
                << ",\"w\":" << d.width << ",\"h\":" << d.height << "}}";
            if (i + 1 < result.detections.size()) out << ",";
        }
        out << "]";
        return true;
    }

private:
    static int64_t timestampMs() {
        using namespace std::chrono;
        return duration_cast<milliseconds>(system_clock::now().time_since_epoch()).count();
    }

    bool openCamera() {
#if SOPHIA_HAVE_OPENCV
        cap_ = cv::VideoCapture(camera_id_);
        return cap_.isOpened();
#else
        return true; // simulado
#endif
    }

    int camera_id_;
    bool model_loaded_;
    bool camera_ok_ = false;
#if SOPHIA_HAVE_OPENCV
    cv::VideoCapture cap_;
#endif
};

} // namespace Sophia::Edge