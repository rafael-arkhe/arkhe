// sophia_vision.cpp — binário de visão em edge (Pi).
#include <chrono>
#include <iostream>
#include <thread>

#include "edge_vision/edge_vision_pipeline.hpp"

int main() {
    using namespace std::chrono_literals;
    Sophia::Edge::EdgeVisionPipeline pipeline(0);
    for (int i = 0; i < 10; ++i) {
        auto result = pipeline.captureAndDetect();
        std::cout << "frame ts=" << result.timestamp_ms
                  << " detections=" << result.detections.size()
                  << " camera=" << (result.camera_ok ? "OK" : "FAIL")
                  << std::endl;
        pipeline.writeDetections("/run/sophia/edge_detections.json", result);
        std::this_thread::sleep_for(500ms);
    }
    return 0;
}