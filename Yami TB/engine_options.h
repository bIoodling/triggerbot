#pragma once
#include "detection_core.h"
struct DetectionOptions {
    bool cursorless=false;
    bool calibrated=false;
    unsigned tolerance=2;
    unsigned cursorRadius=50;
    unsigned fixedRadius=12;
    double clearMs=8;
    unsigned clearFrames=2;
    double minimumIntervalMs=0;
    double normalizedX=.5,normalizedY=.5;
    bool operator==(const DetectionOptions&) const=default;
};
inline std::mutex g_optionsMutex;
inline DetectionOptions g_options;
inline std::atomic<uintptr_t> g_viewportHint{0};
inline std::atomic<bool> g_engineSuspended{false};
inline DetectionOptions ReadDetectionOptions(){std::lock_guard lock(g_optionsMutex);return g_options;}
void ResetEngineConfiguration();
void SetEngineActive(bool active);
void ReplaceDetectionOptions(const DetectionOptions& options);
template<class Edit> void EditDetectionOptions(Edit&& edit){
    auto options=ReadDetectionOptions();edit(options);ReplaceDetectionOptions(options);
}
