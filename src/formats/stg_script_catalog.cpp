#include "formats/stg_script_catalog.h"

namespace kuf {

const ScriptEntryInfo* findConditionInfo(uint32_t id) {
    for (size_t i = 0; i < kConditionCount; ++i) {
        if (kConditions[i].id == id) return &kConditions[i];
    }
    return nullptr;
}

const ScriptEntryInfo* findActionInfo(uint32_t id) {
    for (size_t i = 0; i < kActionCount; ++i) {
        if (kActions[i].id == id) return &kActions[i];
    }
    return nullptr;
}

} // namespace kuf
