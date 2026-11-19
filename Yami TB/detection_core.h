#pragma once
#include <algorithm>
#include <cstdint>
#include <cmath>
#include <mutex>

namespace yami {
enum class Phase { Idle, Armed, Fired, WaitingForClear, Rearmed };
struct Rules {
    double clearMs=8.0;
    unsigned clearFrames=2;
    double minimumIntervalMs=0.0;
};
struct Point {int x=0,y=0;};
struct Viewport {int x=0,y=0,width=0,height=0;};
inline Point ResolvePoint(Viewport v,double nx,double ny) {
    // Nearest-pixel rounding preserves an arbitrary calibrated pixel across
    // normalization/persistence; truncation can drift by one from FP error.
    return {v.x+std::clamp(int(std::lround(std::clamp(nx,0.,1.)*v.width)),0,std::max(0,v.width-1)),
            v.y+std::clamp(int(std::lround(std::clamp(ny,0.,1.)*v.height)),0,std::max(0,v.height-1))};
}
inline bool MatchRgb(uint32_t pixel,uint32_t target,unsigned tolerance) {
    for(unsigned shift:{0u,8u,16u})
        if(std::abs(int((pixel>>shift)&255)-int((target>>shift)&255))>int(tolerance))return false;
    return true;
}
// One synchronous dispatch owner. Release/reset and dispatch serialize through
// this lock. Tokens prevent an old GPU completion crossing any control change.
class SingleTrigger {
    mutable std::mutex mutex_;
    Phase phase_=Phase::Idle;
    bool active_=false,clearStarted_=false;
    uint64_t epoch_=1,lastSequence_=0;
    double clearSince_=0,lastTimestamp_=-1,lastDispatch_=-1e30;
    unsigned clearFrames_=0;
    uint64_t dispatched_=0,failed_=0;
public:
    void SetActive(bool active) {
        std::lock_guard lock(mutex_);
        if(active==active_)return;
        active_=active;++epoch_;phase_=active?Phase::Armed:Phase::Idle;
        clearStarted_=false;clearFrames_=0;lastSequence_=0;lastTimestamp_=-1;
    }
    void Reset() {
        std::lock_guard lock(mutex_);++epoch_;
        phase_=active_?Phase::Armed:Phase::Idle;clearStarted_=false;clearFrames_=0;
        lastSequence_=0;lastTimestamp_=-1;
    }
    template<class Edit> void Configure(Edit&& edit) {
        std::lock_guard lock(mutex_);edit();++epoch_;
        phase_=active_?Phase::Armed:Phase::Idle;clearStarted_=false;clearFrames_=0;
        lastSequence_=0;lastTimestamp_=-1;
    }
    uint64_t Token() const {std::lock_guard lock(mutex_);return epoch_;}
    Phase State() const {std::lock_guard lock(mutex_);return phase_;}
    uint64_t Dispatched() const {std::lock_guard lock(mutex_);return dispatched_;}
    uint64_t Failed() const {std::lock_guard lock(mutex_);return failed_;}
    template<class Allowed,class Dispatch>
    bool Frame(uint64_t token,uint64_t sequence,double timestampMs,bool valid,bool found,
               const Rules& rules,Allowed&& allowed,Dispatch&& dispatch) {
        std::lock_guard lock(mutex_);
        if(token!=epoch_||!active_||sequence<=lastSequence_||timestampMs<lastTimestamp_)return false;
        lastSequence_=sequence;lastTimestamp_=timestampMs;
        if(!valid){clearStarted_=false;clearFrames_=0;return false;}
        if(found){
            clearStarted_=false;clearFrames_=0;
            if(phase_==Phase::WaitingForClear||phase_==Phase::Fired)return false;
            // An appearance during an optional rate limit is consumed, never
            // queued for a delayed trigger when that interval expires.
            if(timestampMs-lastDispatch_<rules.minimumIntervalMs){phase_=Phase::WaitingForClear;return false;}
            if(!allowed()){phase_=Phase::Idle;active_=false;++epoch_;return false;}
            phase_=Phase::Fired;
            bool sent=dispatch();
            if(sent){++dispatched_;lastDispatch_=timestampMs;}else ++failed_;
            // Even a partial/failed SendInput call cannot cause a retry burst.
            phase_=Phase::WaitingForClear;
            return sent;
        }
        if(phase_==Phase::WaitingForClear){
            if(!clearStarted_){clearStarted_=true;clearSince_=timestampMs;clearFrames_=0;}
            ++clearFrames_;
            if(clearFrames_>=std::max(2u,rules.clearFrames)&&timestampMs-clearSince_>=rules.clearMs){
                phase_=Phase::Rearmed;clearStarted_=false;clearFrames_=0;
            }
        }
        return false;
    }
};
} // namespace yami
